//! Audio metadata extraction built on the `lofty` crate.
//!
//! Reads ID3v1/ID3v2, Vorbis comments, FLAC Vorbis, APE and MP4 tags plus
//! stream properties (duration, bitrate, sample rate, channels) and embedded
//! artwork. Artwork is written to a content-addressed cache directory; the
//! SHA-256 hash of the image bytes is used as the lookup key so identical
//! covers are stored once.

use std::path::Path;

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::{Accessor, ItemKey};
use sha2::{Digest, Sha256};

use crate::db::TrackUpsert;
use crate::error::{CoreError, CoreResult};
use crate::util::{parse_gain_db, sniff_image_ext, title_from_filename};

/// Everything extracted from a single audio file, ready to be stored.
#[derive(Debug, Clone)]
pub struct TrackMeta {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub composer: String,
    pub year: Option<i64>,
    pub track_no: Option<i64>,
    pub disc_no: Option<i64>,
    pub duration_ms: i64,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub art_hash: Option<String>,
    pub replay_gain_db: Option<f32>,
    pub lyrics: Option<String>,
    pub lyrics_synced: Option<String>,
}

/// Reads metadata from `path`, caching embedded artwork under `art_cache_dir`.
///
/// Malformed or unsupported files produce a [`CoreError::Metadata`] with a
/// descriptive message; the scanner collects these rather than failing the
/// whole scan.
pub fn read_track_meta(path: &Path, art_cache_dir: &Path) -> CoreResult<TrackMeta> {
    let tagged = lofty::read_from_path(path)
        .map_err(|e| CoreError::Metadata(format!("{}: {e}", path.display())))?;

    let props = tagged.properties();
    let duration_ms = props.duration().as_millis() as i64;
    let bitrate = props.overall_bitrate().map(|b| b as i64);
    let sample_rate = props.sample_rate().map(|s| s as i64);
    let channels = props.channels().map(|c| c as i64);

    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let fallback_title = title_from_filename(path);

    let artist = tag
        .and_then(|t| t.artist().map(|v| v.into_owned()))
        .unwrap_or_default();
    let album = tag
        .and_then(|t| t.album().map(|v| v.into_owned()))
        .unwrap_or_default();
    let title = tag
        .and_then(|t| t.title().map(|v| v.into_owned()))
        .unwrap_or(fallback_title);
    let genre = tag
        .and_then(|t| t.genre().map(|v| v.into_owned()))
        .unwrap_or_default();
    let album_artist = tag
        .and_then(|t| t.get_string(ItemKey::AlbumArtist).map(str::to_string))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| artist.clone());
    let composer = tag
        .and_then(|t| t.get_string(ItemKey::Composer).map(str::to_string))
        .unwrap_or_default();

    let year = tag.and_then(|t| {
        t.get_string(ItemKey::Year)
            .or_else(|| t.get_string(ItemKey::RecordingDate))
            .and_then(|s| s.chars().take(4).collect::<String>().parse::<i64>().ok())
    });

    let track_no = tag.and_then(|t| t.track().map(|n| n as i64));
    let disc_no = tag.and_then(|t| t.disk().map(|n| n as i64));

    let replay_gain_db = tag.and_then(|t| {
        t.get_string(ItemKey::ReplayGainTrackGain)
            .or_else(|| t.get_string(ItemKey::ReplayGainAlbumGain))
            .and_then(parse_gain_db)
    });

    let lyrics = tag.and_then(|t| t.get_string(ItemKey::UnsyncLyrics).map(str::to_string));
    let lyrics_synced = tag.and_then(|t| t.get_string(ItemKey::Lyrics).map(str::to_string));

    let art_hash = tag
        .and_then(|t| {
            let pics = t.pictures();
            pics.iter()
                .find(|p| p.pic_type() == lofty::picture::PictureType::CoverFront)
                .or_else(|| pics.first())
        })
        .map(|pic| cache_artwork(pic.data(), art_cache_dir))
        .transpose()?
        .flatten();

    Ok(TrackMeta {
        title,
        artist,
        album,
        album_artist,
        genre,
        composer,
        year,
        track_no,
        disc_no,
        duration_ms,
        bitrate,
        sample_rate,
        channels,
        art_hash,
        replay_gain_db,
        lyrics,
        lyrics_synced,
    })
}

/// Converts extracted metadata plus a file path into a database upsert payload.
///
/// File size and mtime are read from disk here so every caller (scanner and
/// watcher) produces identical rows.
pub fn to_upsert(path: &Path, meta: TrackMeta) -> TrackUpsert {
    let key = path.to_string_lossy().into_owned();
    let folder = path
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let format = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let (file_size, mtime) = std::fs::metadata(path)
        .map(|m| {
            let mt = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            (m.len() as i64, mt)
        })
        .unwrap_or((0, 0));
    TrackUpsert {
        path: key,
        title: meta.title,
        artist: meta.artist,
        album: meta.album,
        album_artist: meta.album_artist,
        genre: meta.genre,
        composer: meta.composer,
        year: meta.year,
        track_no: meta.track_no,
        disc_no: meta.disc_no,
        duration_ms: meta.duration_ms,
        bitrate: meta.bitrate,
        sample_rate: meta.sample_rate,
        channels: meta.channels,
        format,
        folder,
        file_size,
        mtime,
        art_hash: meta.art_hash,
        replay_gain_db: meta.replay_gain_db,
        lyrics: meta.lyrics,
        lyrics_synced: meta.lyrics_synced,
    }
}

/// Writes artwork bytes into the content-addressed cache, returning the hash.
fn cache_artwork(bytes: &[u8], art_cache_dir: &Path) -> CoreResult<Option<String>> {
    if bytes.is_empty() {
        return Ok(None);
    }
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let hash = hex(&hasher.finalize());
    let ext = sniff_image_ext(bytes);
    let file = art_cache_dir.join(format!("{hash}.{ext}"));
    if !file.exists() {
        std::fs::create_dir_all(art_cache_dir)?;
        std::fs::write(&file, bytes)?;
    }
    Ok(Some(hash))
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A minimal valid WAV file with a tiny sine wave, plus a "TIT2"-style
    /// RIFF INFO tag is hard to synthesize portably, so the test focuses on
    /// the parts we control: error handling and artwork caching.
    fn write_wav(path: &Path) {
        let sample_rate = 8000u32;
        let seconds = 1u32;
        let n = sample_rate * seconds;
        let mut data = Vec::with_capacity((44 + n * 2) as usize);
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&(36 + n * 2).to_le_bytes());
        data.extend_from_slice(b"WAVEfmt ");
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes()); // PCM
        data.extend_from_slice(&1u16.to_le_bytes()); // mono
        data.extend_from_slice(&sample_rate.to_le_bytes());
        data.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        data.extend_from_slice(&2u16.to_le_bytes());
        data.extend_from_slice(&16u16.to_le_bytes());
        data.extend_from_slice(b"data");
        data.extend_from_slice(&(n * 2).to_le_bytes());
        for i in 0..n {
            let sample = (i as f32 * 2.0 * std::f32::consts::PI * 440.0 / sample_rate as f32).sin();
            data.extend_from_slice(&((sample * i16::MAX as f32) as i16).to_le_bytes());
        }
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(&data).unwrap();
    }

    #[test]
    fn reads_wav_properties() {
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("tone.wav");
        write_wav(&wav);
        let meta = read_track_meta(&wav, dir.path()).unwrap();
        assert_eq!(meta.title, "tone"); // falls back to the file stem
        assert!(meta.duration_ms > 0);
    }

    #[test]
    fn missing_file_is_an_error_not_a_panic() {
        let err = read_track_meta(Path::new("/nonexistent/x.mp3"), Path::new("/tmp"));
        assert!(err.is_err());
    }

    #[test]
    fn garbage_file_is_handled_gracefully() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("garbage.mp3");
        std::fs::write(&bad, b"this is not audio data at all").unwrap();
        let err = read_track_meta(&bad, dir.path());
        assert!(err.is_err());
    }

    #[test]
    fn artwork_cache_is_content_addressed() {
        let dir = tempfile::tempdir().unwrap();
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];
        let h = cache_artwork(&png, dir.path()).unwrap().unwrap();
        assert_eq!(h.len(), 64);
        let file = dir.path().join(format!("{h}.png"));
        assert!(file.exists());
        // Caching the same bytes returns the same key without a second write.
        let h2 = cache_artwork(&png, dir.path()).unwrap().unwrap();
        assert_eq!(h, h2);
    }
}
