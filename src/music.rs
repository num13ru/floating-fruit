use std::{fmt, path::PathBuf, process::Command};

use anyhow::{Context, Result, anyhow};

const FIELD_SEP: char = '\u{001f}';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Playing,
    Paused,
    FastForwarding,
    Rewinding,
}

impl fmt::Display for PlayerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Playing => write!(f, "playing"),
            Self::Paused => write!(f, "paused"),
            Self::FastForwarding => write!(f, "fast forwarding"),
            Self::Rewinding => write!(f, "rewinding"),
        }
    }
}

impl PlayerState {
    fn parse(s: &str) -> Result<Self> {
        match s {
            "playing" => Ok(Self::Playing),
            "paused" => Ok(Self::Paused),
            "fast forwarding" => Ok(Self::FastForwarding),
            "rewinding" => Ok(Self::Rewinding),
            other => Err(anyhow!("Unknown player state: {:?}", other)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackInfo {
    pub state: PlayerState,
    pub title: String,
    pub artist: String,
    pub artwork_path: Option<PathBuf>,
    pub position: f64,
    pub duration: f64,
}

pub fn query() -> Result<Option<TrackInfo>> {
    let base_path = std::env::temp_dir().join("am_now_playing_artwork");
    let base_path_str = base_path
        .to_str()
        .ok_or_else(|| anyhow!("Temp path is not valid UTF-8"))?;

    let script = format!(
        r#"
if application "Music" is not running then
    return "stopped"
end if

set sep to character id 31
set outputBase to "{output_base}"

tell application "Music"
    if player state is stopped then
        return "stopped"
    end if

    set t to current track
    set trackState to (player state as text)
    set trackTitle to (name of t as text)
    set trackArtist to (artist of t as text)
    set artPath to ""

    try
        if (count of artworks of t) > 0 then
            tell artwork 1 of t
                if format is JPEG picture then
                    set imgExt to ".jpg"
                else
                    set imgExt to ".png"
                end if
            end tell

            set rawData to (get raw data of artwork 1 of t)
            set artPath to outputBase & imgExt

            set fileRef to open for access (POSIX file artPath) with write permission
            set eof fileRef to 0
            write rawData to fileRef
            close access fileRef
        end if
    on error
        try
            close access fileRef
        end try
        set artPath to ""
    end try

    set trackPos to (player position as text)
    set trackDur to (duration of t as text)

    set AppleScript's text item delimiters to sep
    return {{trackState, trackTitle, trackArtist, artPath, trackPos, trackDur}} as text
end tell
"#,
        output_base = escape_applescript_string(base_path_str),
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .context("Failed to run osascript")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("osascript failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if stdout == "stopped" || stdout.is_empty() {
        return Ok(None);
    }

    let parts: Vec<&str> = stdout.split(FIELD_SEP).collect();
    if parts.len() != 6 {
        return Err(anyhow!("Unexpected AppleScript output: {:?}", stdout));
    }

    let artwork_path = if parts[3].trim().is_empty() {
        None
    } else {
        Some(PathBuf::from(parts[3].trim()))
    };

    let position: f64 = parts[4]
        .trim()
        .parse()
        .with_context(|| format!("Failed to parse position: {:?}", parts[4]))?;
    let duration: f64 = parts[5]
        .trim()
        .parse()
        .with_context(|| format!("Failed to parse duration: {:?}", parts[5]))?;

    Ok(Some(TrackInfo {
        state: PlayerState::parse(parts[0].trim())?,
        title: parts[1].trim().to_string(),
        artist: parts[2].trim().to_string(),
        artwork_path,
        position,
        duration,
    }))
}

pub enum PlayerCommand {
    Previous,
    PlayPause,
    Next,
}

pub fn send_command(cmd: PlayerCommand) -> Result<()> {
    let script = match cmd {
        PlayerCommand::Previous => r#"tell application "Music" to previous track"#,
        PlayerCommand::PlayPause => r#"tell application "Music" to playpause"#,
        PlayerCommand::Next => r#"tell application "Music" to next track"#,
    };

    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .context("Failed to run osascript")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("osascript failed: {}", stderr.trim()));
    }

    Ok(())
}

fn escape_applescript_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}
