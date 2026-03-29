use std::{path::PathBuf, process::Command};

use anyhow::{Context, Result, anyhow};

const FIELD_SEP: char = '\u{001f}';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Playing,
    Paused,
    FastForwarding,
    Rewinding,
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
    pub id: i64,
    pub position: f64,
    pub duration: f64,
}

pub fn query() -> Result<Option<TrackInfo>> {
    let script = r#"
if application "Music" is not running then
    return "stopped"
end if

set sep to character id 31

tell application "Music"
    if player state is stopped then
        return "stopped"
    end if

    set t to current track
    set trackState to (player state as text)
    set trackTitle to (name of t as text)
    set trackArtist to (artist of t as text)
    set trackId to (id of t as text)
    set trackPos to (player position as text)
    set trackDur to (duration of t as text)

    set AppleScript's text item delimiters to sep
    return {trackState, trackTitle, trackArtist, trackId, trackPos, trackDur} as text
end tell
"#;

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

    let id: i64 = parts[3]
        .trim()
        .parse()
        .with_context(|| format!("Failed to parse track id: {:?}", parts[3]))?;
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
        id,
        position,
        duration,
    }))
}

pub fn extract_artwork() -> Result<Option<PathBuf>> {
    let base_path = std::env::temp_dir().join("am_now_playing_artwork");
    let base_path_str = base_path
        .to_str()
        .ok_or_else(|| anyhow!("Temp path is not valid UTF-8"))?;

    let script = format!(
        r#"
tell application "Music"
    set t to current track
    set outputBase to "{output_base}"
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

    return artPath
end tell
"#,
        output_base = escape_applescript_string(base_path_str),
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .context("Failed to run osascript")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("osascript artwork failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if stdout.is_empty() {
        Ok(None)
    } else {
        Ok(Some(PathBuf::from(stdout)))
    }
}

pub enum PlayerCommand {
    Previous,
    PlayPause,
    Next,
    Seek(f64),
}

pub fn send_command(cmd: PlayerCommand) -> Result<()> {
    let script: String = match cmd {
        PlayerCommand::Previous => r#"tell application "Music" to previous track"#.into(),
        PlayerCommand::PlayPause => r#"tell application "Music" to playpause"#.into(),
        PlayerCommand::Next => r#"tell application "Music" to next track"#.into(),
        PlayerCommand::Seek(pos) => {
            anyhow::ensure!(pos.is_finite(), "Seek position must be finite, got {pos}");
            format!(r#"tell application "Music" to set player position to {pos}"#)
        }
    };

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
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
