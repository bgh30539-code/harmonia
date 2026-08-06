use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Extensions recognised by the library scanner and playback layer.
pub const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "ogg", "opus", "wav", "m4a", "aac", "mp4"];

/// Returns the current Unix timestamp in seconds.
pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// True when `path` has one of the known audio extensions (case-insensitive).
pub fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Formats a millisecond duration as "m:ss" or "h:mm:ss".
pub fn format_duration(ms: i64) -> String {
    let total = ms.max(0) / 1000;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Escapes LIKE wildcards so user input is matched literally.
pub fn escape_like(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Parses a ReplayGain value such as "+1.23 dB", "-4.5 dB" or "0.00".
pub fn parse_gain_db(raw: &str) -> Option<f32> {
    let trimmed = raw.trim();
    let trimmed = trimmed
        .strip_suffix("dB")
        .or_else(|| trimmed.strip_suffix("db"))
        .or_else(|| trimmed.strip_suffix("DB"))
        .unwrap_or(trimmed)
        .trim();
    let cleaned: String = trimmed
        .chars()
        .filter(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+'))
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    cleaned.replace(',', ".").parse::<f32>().ok()
}

/// Sniffs the image container from magic bytes and returns a file extension.
pub fn sniff_image_ext(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        "png"
    } else if bytes.starts_with(b"GIF8") {
        "gif"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "webp"
    } else {
        "jpg"
    }
}

/// Fallback display name for a file without readable tags.
pub fn title_from_filename(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_formatting() {
        assert_eq!(format_duration(0), "0:00");
        assert_eq!(format_duration(65_000), "1:05");
        assert_eq!(format_duration(3_661_000), "1:01:01");
    }

    #[test]
    fn gain_parsing() {
        assert_eq!(parse_gain_db("+1.23 dB"), Some(1.23));
        assert_eq!(parse_gain_db("-4.5 dB"), Some(-4.5));
        assert_eq!(parse_gain_db("0.00"), Some(0.0));
        assert_eq!(parse_gain_db("nope"), None);
        assert_eq!(parse_gain_db("-2,75 dB"), Some(-2.75));
    }

    #[test]
    fn like_escaping() {
        assert_eq!(escape_like("50%_off"), r"50\%\_off");
    }

    #[test]
    fn image_sniffing() {
        assert_eq!(sniff_image_ext(&[0xFF, 0xD8, 0xFF, 0xE0]), "jpg");
        assert_eq!(
            sniff_image_ext(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
            "png"
        );
        assert_eq!(sniff_image_ext(&[1, 2, 3]), "jpg");
    }

    #[test]
    fn audio_detection() {
        assert!(is_audio_file(Path::new("/m/song.MP3")));
        assert!(is_audio_file(Path::new("/m/song.flac")));
        assert!(!is_audio_file(Path::new("/m/cover.jpg")));
        assert!(!is_audio_file(Path::new("/m/notes.txt")));
    }
}
