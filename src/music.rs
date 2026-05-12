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
    pub album: String,
    pub album_artist: String,
    pub id: i64,
    pub position: f64,
    pub duration: f64,
}

impl TrackInfo {
    pub fn album_key(&self) -> String {
        format!("{}\0{}", self.album_artist, self.album)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtworkRequest {
    pub track_id: i64,
    pub album_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtworkExtraction {
    Path(PathBuf),
    Missing,
    Stale,
}

impl ArtworkExtraction {
    fn parse(stdout: &str) -> Self {
        match stdout {
            "" => Self::Missing,
            "__STALE__" => Self::Stale,
            path => Self::Path(PathBuf::from(path)),
        }
    }
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
    set trackAlbum to (album of t as text)
    set trackAlbumArtist to (album artist of t as text)
    set trackId to (id of t as text)
    set trackPos to (player position as text)
    set trackDur to (duration of t as text)

    set AppleScript's text item delimiters to sep
    return {trackState, trackTitle, trackArtist, trackAlbum, trackAlbumArtist, trackId, trackPos, trackDur} as text
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
    if parts.len() != 8 {
        return Err(anyhow!("Unexpected AppleScript output: {:?}", stdout));
    }

    let id: i64 = parts[5]
        .trim()
        .parse()
        .with_context(|| format!("Failed to parse track id: {:?}", parts[5]))?;
    let position: f64 = parts[6]
        .trim()
        .parse()
        .with_context(|| format!("Failed to parse position: {:?}", parts[6]))?;
    let duration: f64 = parts[7]
        .trim()
        .parse()
        .with_context(|| format!("Failed to parse duration: {:?}", parts[7]))?;

    Ok(Some(TrackInfo {
        state: PlayerState::parse(parts[0].trim())?,
        title: parts[1].trim().to_string(),
        artist: parts[2].trim().to_string(),
        album: parts[3].trim().to_string(),
        album_artist: parts[4].trim().to_string(),
        id,
        position,
        duration,
    }))
}

pub fn extract_artwork(request: &ArtworkRequest) -> Result<ArtworkExtraction> {
    let base_path =
        std::env::temp_dir().join(format!("am_now_playing_artwork_{}", request.track_id));
    let base_path_str = base_path
        .to_str()
        .ok_or_else(|| anyhow!("Temp path is not valid UTF-8"))?;
    let expected_track_id = AppleScriptNumber(request.track_id);

    let script = format!(
        r#"
tell application "Music"
    set t to current track
    if (id of t as text) is not "{expected_track_id}" then
        return "__STALE__"
    end if

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
        expected_track_id = expected_track_id,
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

    Ok(ArtworkExtraction::parse(&stdout))
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

pub fn reveal_current_track() -> Result<()> {
    let script = r#"
tell application "Music"
    activate
    reveal current track
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

    Ok(())
}

fn escape_applescript_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

struct AppleScriptNumber(i64);

impl fmt::Display for AppleScriptNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{ArtworkExtraction, PlayerState, TrackInfo};

    #[test]
    fn artwork_extraction_parse_distinguishes_stale_from_missing() {
        assert_eq!(ArtworkExtraction::parse(""), ArtworkExtraction::Missing);
        assert_eq!(
            ArtworkExtraction::parse("__STALE__"),
            ArtworkExtraction::Stale
        );
    }

    #[test]
    fn album_key_uses_album_artist_and_album() {
        let track = TrackInfo {
            state: PlayerState::Playing,
            title: "Title".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            album_artist: "Album Artist".into(),
            id: 1,
            position: 0.0,
            duration: 10.0,
        };

        assert_eq!(track.album_key(), "Album Artist\0Album");
    }
}
