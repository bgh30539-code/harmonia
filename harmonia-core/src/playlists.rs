//! Playlist persistence and interchange.
//!
//! Exports to M3U/M3U8, PLS and XSPF. Imports M3U/M3U8 (the most common
//! interchange format), resolving relative paths against the playlist file
//! location and decoding `file://` URIs. Smart playlists are stored as JSON
//! rules and evaluated here into parameterised SQL.

use std::path::Path;

use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};
use rusqlite::types::Value;

use crate::error::{CoreError, CoreResult};
use crate::models::{SmartRules, Track};

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Characters that must be percent-encoded inside a file:// URI.
/// Slashes, colons and dots are preserved.
const FILE_URI_ENCODE: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'<')
    .add(b'>')
    .add(b'\\')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}')
    .add(b'%')
    .add(b'#')
    .add(b'?');

fn file_uri(path: &str) -> String {
    format!("file://{}", utf8_percent_encode(path, FILE_URI_ENCODE))
}

/// Writes `tracks` to `path` in the requested format:
/// "m3u" | "m3u8" | "pls" | "xspf".
pub fn export_playlist(path: &Path, format: &str, name: &str, tracks: &[Track]) -> CoreResult<()> {
    match format.to_ascii_lowercase().as_str() {
        "m3u" | "m3u8" => {
            let mut out = String::from("#EXTM3U\n");
            for t in tracks {
                out.push_str(&format!(
                    "#EXTINF:{},\"{}\" - {}\n{}\n",
                    t.duration_ms / 1000,
                    t.artist,
                    t.title,
                    t.path
                ));
            }
            std::fs::write(path, out)?;
        }
        "pls" => {
            let mut out = String::from("[playlist]\n");
            for (i, t) in tracks.iter().enumerate() {
                let n = i + 1;
                out.push_str(&format!("File{n}={}\n", t.path));
                out.push_str(&format!("Title{n}={} - {}\n", t.artist, t.title));
                out.push_str(&format!("Length{n}={}\n", t.duration_ms / 1000));
            }
            out.push_str(&format!("NumberOfEntries={}\nVersion=2\n", tracks.len()));
            std::fs::write(path, out)?;
        }
        "xspf" => {
            let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
            out.push_str("<playlist version=\"1\" xmlns=\"http://xspf.org/ns/0/\">\n");
            out.push_str(&format!("  <title>{}</title>\n", xml_escape(name)));
            out.push_str("  <trackList>\n");
            for t in tracks {
                out.push_str("    <track>\n");
                out.push_str(&format!(
                    "      <location>{}</location>\n",
                    file_uri(&t.path)
                ));
                out.push_str(&format!("      <title>{}</title>\n", xml_escape(&t.title)));
                out.push_str(&format!(
                    "      <creator>{}</creator>\n",
                    xml_escape(&t.artist)
                ));
                out.push_str(&format!("      <album>{}</album>\n", xml_escape(&t.album)));
                out.push_str(&format!("      <duration>{}</duration>\n", t.duration_ms));
                out.push_str("    </track>\n");
            }
            out.push_str("  </trackList>\n</playlist>\n");
            std::fs::write(path, out)?;
        }
        other => {
            return Err(CoreError::Invalid(format!(
                "unsupported playlist format: {other}"
            )));
        }
    }
    Ok(())
}

/// Reads an M3U/M3U8 file and returns absolute file paths.
pub fn import_m3u(path: &Path) -> CoreResult<Vec<String>> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| CoreError::Playlist(format!("cannot read {}: {e}", path.display())))?;
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let mut out = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let resolved = if let Some(rest) = line.strip_prefix("file://") {
            percent_decode_str(rest)
                .decode_utf8()
                .map(|c| c.into_owned())
                .unwrap_or_else(|_| rest.to_string())
        } else if Path::new(line).is_absolute() {
            line.to_string()
        } else {
            base.join(line).to_string_lossy().into_owned()
        };
        out.push(resolved);
    }
    Ok(out)
}

/// Returns the set of file paths from an import that exist in the library.
pub fn match_imported_paths(
    db: &crate::db::Database,
    paths: &[String],
) -> CoreResult<Vec<(String, Track)>> {
    let mut matched = Vec::new();
    for p in paths {
        if let Some(track) = db.get_track_by_path(p)? {
            matched.push((p.clone(), track));
        }
    }
    Ok(matched)
}

// ---------------------------------------------------------------------------
// Smart playlists
// ---------------------------------------------------------------------------

/// Maps a rule field name to its SQL column. Unknown fields are rejected so
/// user input can never be interpolated into SQL.
fn field_column(field: &str) -> Option<&'static str> {
    match field {
        "genre" => Some("genre"),
        "artist" => Some("artist"),
        "album" => Some("album"),
        "composer" => Some("composer"),
        "format" => Some("format"),
        "year" => Some("year"),
        "playCount" => Some("play_count"),
        "favorite" => Some("favorite"),
        "durationMs" => Some("duration_ms"),
        "bitrate" => Some("bitrate"),
        _ => None,
    }
}

fn is_numeric_field(field: &str) -> bool {
    matches!(
        field,
        "year" | "playCount" | "favorite" | "durationMs" | "bitrate"
    )
}

fn coerce_value(field: &str, raw: &str) -> Value {
    if is_numeric_field(field) {
        let n = match field {
            "favorite" => match raw.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" => 1,
                _ => 0,
            },
            _ => raw.trim().parse::<i64>().unwrap_or(0),
        };
        Value::Integer(n)
    } else {
        Value::Text(raw.to_string())
    }
}

/// Builds a parameterised WHERE clause for a set of smart playlist rules.
pub fn smart_playlist_where(rules: &SmartRules) -> CoreResult<(String, Vec<Value>)> {
    if rules.rules.is_empty() {
        return Err(CoreError::Invalid(
            "smart playlist requires at least one rule".into(),
        ));
    }
    let mut clauses: Vec<String> = Vec::new();
    let mut params: Vec<Value> = Vec::new();

    for rule in &rules.rules {
        let column = field_column(&rule.field).ok_or_else(|| {
            CoreError::Invalid(format!("unknown smart playlist field: {}", rule.field))
        })?;
        let numeric = is_numeric_field(&rule.field);
        match rule.op.as_str() {
            "eq" => {
                clauses.push(format!("{column} = ?"));
                params.push(coerce_value(&rule.field, &rule.value));
            }
            "ne" => {
                clauses.push(format!("{column} != ?"));
                params.push(coerce_value(&rule.field, &rule.value));
            }
            "contains" => {
                let like = format!("%{}%", crate::util::escape_like(&rule.value));
                if numeric {
                    clauses.push(format!("CAST({column} AS TEXT) LIKE ? ESCAPE '\\'"));
                } else {
                    clauses.push(format!("{column} LIKE ? ESCAPE '\\'"));
                }
                params.push(Value::Text(like));
            }
            "gt" | "gte" | "lt" | "lte" if numeric => {
                let op = match rule.op.as_str() {
                    "gt" => ">",
                    "gte" => ">=",
                    "lt" => "<",
                    _ => "<=",
                };
                clauses.push(format!("{column} {op} ?"));
                params.push(coerce_value(&rule.field, &rule.value));
            }
            other => {
                return Err(CoreError::Invalid(format!(
                    "unsupported smart playlist operator: {other}"
                )));
            }
        }
    }

    let sep = if rules.match_all { " AND " } else { " OR " };
    Ok((clauses.join(sep), params))
}

/// Validates a rule set without running it (used before persisting).
pub fn validate_rules(rules: &SmartRules) -> CoreResult<()> {
    let _ = smart_playlist_where(rules)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RepeatMode, SmartRule};
    use crate::settings::Settings;

    fn track(path: &str, artist: &str, title: &str, ms: i64) -> Track {
        Track {
            id: 0,
            path: path.into(),
            title: title.into(),
            artist: artist.into(),
            album: "Al".into(),
            album_artist: artist.into(),
            genre: "Rock".into(),
            composer: String::new(),
            year: Some(2020),
            track_no: Some(1),
            disc_no: Some(1),
            duration_ms: ms,
            bitrate: Some(320),
            sample_rate: Some(44_100),
            channels: Some(2),
            format: "mp3".into(),
            folder: "/music".into(),
            art_hash: None,
            favorite: false,
            play_count: 0,
            last_played: None,
            date_added: 0,
            replay_gain_db: None,
            lyrics: None,
            lyrics_synced: None,
        }
    }

    fn rule(field: &str, op: &str, value: &str) -> SmartRule {
        SmartRule {
            field: field.into(),
            op: op.into(),
            value: value.into(),
        }
    }

    // ------------------------------------------------------------------
    // M3U / M3U8
    // ------------------------------------------------------------------

    #[test]
    fn m3u_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let m3u = dir.path().join("mix.m3u");
        let tracks = vec![track("/music/a.mp3", "A", "One", 65_000)];
        export_playlist(&m3u, "m3u", "Mix", &tracks).unwrap();
        let content = std::fs::read_to_string(&m3u).unwrap();
        assert!(content.starts_with("#EXTM3U"));
        assert!(content.contains("#EXTINF:65,"));
        assert!(content.contains("/music/a.mp3"));
        let imported = import_m3u(&m3u).unwrap();
        assert_eq!(imported, vec!["/music/a.mp3".to_string()]);
    }

    #[test]
    fn m3u8_is_an_alias_for_m3u() {
        let dir = tempfile::tempdir().unwrap();
        let tracks = vec![track("/music/a.mp3", "A", "One", 65_000)];
        let m3u = dir.path().join("mix.m3u");
        let m3u8 = dir.path().join("mix.m3u8");
        export_playlist(&m3u, "m3u", "Mix", &tracks).unwrap();
        export_playlist(&m3u8, "m3u8", "Mix", &tracks).unwrap();
        let m3u_content = std::fs::read_to_string(&m3u).unwrap();
        let m3u8_content = std::fs::read_to_string(&m3u8).unwrap();
        assert_eq!(
            m3u_content, m3u8_content,
            "m3u8 must be byte-identical to m3u"
        );
        assert!(m3u8_content.contains("#EXTINF:65,\"A\" - One"));
    }

    #[test]
    fn m3u_export_multiple_tracks_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let m3u = dir.path().join("mix.m3u");
        let tracks = vec![
            track("/music/a.mp3", "A", "One", 65_000),
            track("/music/b.flac", "B", "Two", 130_000),
        ];
        export_playlist(&m3u, "m3u", "Mix", &tracks).unwrap();
        let content = std::fs::read_to_string(&m3u).unwrap();
        let a = content.find("/music/a.mp3").unwrap();
        let b = content.find("/music/b.flac").unwrap();
        assert!(a < b, "tracks must be exported in order");
        assert!(content.contains("#EXTINF:130,\"B\" - Two"));
    }

    #[test]
    fn m3u_import_skips_comments_and_blank_lines() {
        let dir = tempfile::tempdir().unwrap();
        let m3u = dir.path().join("skip.m3u");
        std::fs::write(
            &m3u,
            "#EXTM3U\n#EXTINF:65,A - One\n/music/a.mp3\n\n# just a comment\n/music/b.mp3\n",
        )
        .unwrap();
        let imported = import_m3u(&m3u).unwrap();
        assert_eq!(
            imported,
            vec!["/music/a.mp3".to_string(), "/music/b.mp3".to_string()]
        );
    }

    #[test]
    fn m3u_import_absolute_paths_passthrough() {
        let dir = tempfile::tempdir().unwrap();
        let m3u = dir.path().join("abs.m3u");
        std::fs::write(&m3u, "/music/song.mp3\n").unwrap();
        let imported = import_m3u(&m3u).unwrap();
        assert_eq!(imported, vec!["/music/song.mp3".to_string()]);
    }

    #[test]
    fn relative_and_uri_paths_resolve() {
        let dir = tempfile::tempdir().unwrap();
        let m3u = dir.path().join("rel.m3u");
        std::fs::write(&m3u, "song.mp3\n").unwrap();
        let imported = import_m3u(&m3u).unwrap();
        assert_eq!(
            imported,
            vec![dir.path().join("song.mp3").to_string_lossy().into_owned()]
        );
    }

    #[test]
    fn m3u_import_resolves_nested_relative_paths() {
        let dir = tempfile::tempdir().unwrap();
        let m3u = dir.path().join("nested.m3u");
        std::fs::write(&m3u, "sub/dir/song.mp3\n").unwrap();
        let imported = import_m3u(&m3u).unwrap();
        assert_eq!(
            imported,
            vec![dir
                .path()
                .join("sub/dir/song.mp3")
                .to_string_lossy()
                .into_owned()]
        );
    }

    #[test]
    fn m3u_import_decodes_percent_encoded_file_uris() {
        let dir = tempfile::tempdir().unwrap();
        let m3u = dir.path().join("uri.m3u");
        // Spaces (%20), hashes (%23) and literal percent signs (%25) must be
        // decoded back into the original path.
        std::fs::write(&m3u, "file:///music/My%20Song%20%231.mp3\n").unwrap();
        let imported = import_m3u(&m3u).unwrap();
        assert_eq!(imported, vec!["/music/My Song #1.mp3".to_string()]);
    }

    #[test]
    fn m3u_import_handles_malformed_utf8_gracefully() {
        let dir = tempfile::tempdir().unwrap();
        let m3u = dir.path().join("bad.m3u");
        // %FF is not valid UTF-8; the decoder must not panic and must fall
        // back to the raw (still percent-encoded) rest of the URI.
        std::fs::write(&m3u, "file:///music/%FF%FE.mp3\n").unwrap();
        let imported = import_m3u(&m3u).unwrap();
        assert_eq!(imported, vec!["/music/%FF%FE.mp3".to_string()]);
    }

    #[test]
    fn m3u_import_file_uri_with_host_prefix_is_kept_raw() {
        let dir = tempfile::tempdir().unwrap();
        let m3u = dir.path().join("host.m3u");
        // "file://localhost/..." is not special-cased; the host component is
        // preserved verbatim after stripping the "file://" prefix.
        std::fs::write(&m3u, "file://localhost/music/a.mp3\n").unwrap();
        let imported = import_m3u(&m3u).unwrap();
        assert_eq!(imported, vec!["localhost/music/a.mp3".to_string()]);
    }

    #[test]
    fn m3u_import_missing_file_is_playlist_error() {
        let err = import_m3u(Path::new("/nonexistent/nope.m3u")).unwrap_err();
        assert!(matches!(err, CoreError::Playlist(_)));
    }

    // ------------------------------------------------------------------
    // PLS
    // ------------------------------------------------------------------

    #[test]
    fn pls_export_writes_valid_structure() {
        let dir = tempfile::tempdir().unwrap();
        let pls = dir.path().join("mix.pls");
        let tracks = vec![track("/music/a.mp3", "A", "One", 65_000)];
        export_playlist(&pls, "pls", "Mix", &tracks).unwrap();
        let content = std::fs::read_to_string(&pls).unwrap();
        assert!(content.starts_with("[playlist]\n"));
        assert!(content.contains("File1=/music/a.mp3\n"));
        assert!(content.contains("Title1=A - One\n"));
        assert!(content.contains("Length1=65\n"));
        assert!(content.ends_with("NumberOfEntries=1\nVersion=2\n"));
    }

    #[test]
    fn pls_export_counts_multiple_entries() {
        let dir = tempfile::tempdir().unwrap();
        let pls = dir.path().join("two.pls");
        let tracks = vec![
            track("/music/a.mp3", "A", "One", 65_000),
            track("/music/b.flac", "B", "Two", 130_000),
        ];
        export_playlist(&pls, "pls", "Two", &tracks).unwrap();
        let content = std::fs::read_to_string(&pls).unwrap();
        assert!(content.contains("File1=/music/a.mp3\n"));
        assert!(content.contains("File2=/music/b.flac\n"));
        assert!(content.contains("Title2=B - Two\n"));
        assert!(content.contains("Length2=130\n"));
        assert!(content.contains("NumberOfEntries=2\n"));
    }

    #[test]
    fn pls_export_empty_playlist() {
        let dir = tempfile::tempdir().unwrap();
        let pls = dir.path().join("empty.pls");
        export_playlist(&pls, "pls", "Empty", &[]).unwrap();
        let content = std::fs::read_to_string(&pls).unwrap();
        assert!(content.ends_with("NumberOfEntries=0\nVersion=2\n"));
    }

    // ------------------------------------------------------------------
    // XSPF
    // ------------------------------------------------------------------

    #[test]
    fn xspf_export_uses_millisecond_durations() {
        let dir = tempfile::tempdir().unwrap();
        let xspf = dir.path().join("out.xspf");
        let tracks = vec![track("/music/a.mp3", "A", "One", 65_000)];
        export_playlist(&xspf, "xspf", "Mix", &tracks).unwrap();
        let content = std::fs::read_to_string(&xspf).unwrap();
        // XSPF durations are milliseconds (unlike M3U/PLS seconds).
        assert!(content.contains("<duration>65000</duration>"));
        assert!(!content.contains("<duration>65</duration>"));
    }

    #[test]
    fn xspf_escapes_xml() {
        let dir = tempfile::tempdir().unwrap();
        let xspf = dir.path().join("out.xspf");
        let mut t = track("/music/a.mp3", "A&B", "One <Two>", 65_000);
        t.title = "One <Two> & More".into();
        export_playlist(&xspf, "xspf", "Mix & Match", &[t]).unwrap();
        let content = std::fs::read_to_string(&xspf).unwrap();
        assert!(content.contains("&amp;"));
        assert!(content.contains("&lt;"));
        assert!(content.contains("<location>file:///music/a.mp3</location>"));
        // Every XML-special character in title/creator/album/name is escaped.
        assert!(content.contains("<title>One &lt;Two&gt; &amp; More</title>"));
        assert!(content.contains("<creator>A&amp;B</creator>"));
        assert!(content.contains("<title>Mix &amp; Match</title>"));
    }

    #[test]
    fn xspf_escapes_all_five_xml_characters() {
        let dir = tempfile::tempdir().unwrap();
        let xspf = dir.path().join("quotes.xspf");
        let mut t = track("/music/a.mp3", "A", "Say \"hi\"", 1000);
        t.album = "It's <&> \"quoted\"".into();
        export_playlist(&xspf, "xspf", "Name 'with' \"quotes\"", &[t]).unwrap();
        let content = std::fs::read_to_string(&xspf).unwrap();
        assert!(content.contains("Say &quot;hi&quot;"));
        assert!(content.contains("It&apos;s &lt;&amp;&gt; &quot;quoted&quot;"));
        assert!(content.contains("Name &apos;with&apos; &quot;quotes&quot;"));
    }

    #[test]
    fn file_uri_encodes_special_characters() {
        // The FILE_URI_ENCODE set: spaces, #, ?, %, quotes, <>, backslash, etc.
        assert_eq!(file_uri("/music/a b.mp3"), "file:///music/a%20b.mp3");
        assert_eq!(file_uri("/music/#1.mp3"), "file:///music/%231.mp3");
        // '=' is a valid URI sub-delim and stays raw; '?' is encoded.
        assert_eq!(file_uri("/music/q?x=1.mp3"), "file:///music/q%3Fx=1.mp3");
        assert_eq!(file_uri("/music/50%.mp3"), "file:///music/50%25.mp3");
        assert_eq!(file_uri("/music/a\\b.mp3"), "file:///music/a%5Cb.mp3");
        assert_eq!(
            file_uri("/music/{x}|y^z.mp3"),
            "file:///music/%7Bx%7D%7Cy%5Ez.mp3"
        );
        // Slashes, colons, equals and dots are preserved.
        assert_eq!(file_uri("/music/a.b/c.mp3"), "file:///music/a.b/c.mp3");
    }

    #[test]
    fn xspf_export_uri_escapes_spaces_and_specials() {
        let dir = tempfile::tempdir().unwrap();
        let xspf = dir.path().join("spaces.xspf");
        let t = track("/music/My Song #1.mp3", "A", "One", 65_000);
        export_playlist(&xspf, "xspf", "Mix", &[t]).unwrap();
        let content = std::fs::read_to_string(&xspf).unwrap();
        assert!(content.contains("<location>file:///music/My%20Song%20%231.mp3</location>"));
    }

    #[test]
    fn export_rejects_unknown_formats() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.wpl");
        let err = export_playlist(&out, "wpl", "Mix", &[]).unwrap_err();
        assert!(matches!(err, CoreError::Invalid(_)));
        assert!(
            !out.exists(),
            "no file should be written for unknown formats"
        );
    }

    // ------------------------------------------------------------------
    // Smart playlists
    // ------------------------------------------------------------------

    #[test]
    fn smart_rules_build_valid_sql() {
        let rules = SmartRules {
            match_all: true,
            rules: vec![
                SmartRule {
                    field: "genre".into(),
                    op: "eq".into(),
                    value: "Jazz".into(),
                },
                SmartRule {
                    field: "year".into(),
                    op: "gte".into(),
                    value: "2000".into(),
                },
            ],
        };
        let (sql, params) = smart_playlist_where(&rules).unwrap();
        assert!(sql.contains("genre = ?"));
        assert!(sql.contains("year >= ?"));
        assert_eq!(params.len(), 2);
        assert_eq!(params[0], Value::Text("Jazz".into()));
        assert_eq!(params[1], Value::Integer(2000));
    }

    #[test]
    fn smart_rules_join_with_or_when_not_match_all() {
        let rules = SmartRules {
            match_all: false,
            rules: vec![rule("genre", "eq", "Jazz"), rule("genre", "eq", "Rock")],
        };
        let (sql, params) = smart_playlist_where(&rules).unwrap();
        assert!(sql.contains(" OR "), "expected OR join, got: {sql}");
        assert!(!sql.contains(" AND "), "expected no AND join, got: {sql}");
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn smart_rules_support_all_comparison_operators() {
        for op in ["eq", "ne", "gt", "gte", "lt", "lte"] {
            let rules = SmartRules {
                match_all: true,
                rules: vec![rule("year", op, "2000")],
            };
            let (sql, _) = smart_playlist_where(&rules).unwrap();
            let expected = match op {
                "eq" => "year = ?",
                "ne" => "year != ?",
                "gt" => "year > ?",
                "gte" => "year >= ?",
                "lt" => "year < ?",
                _ => "year <= ?",
            };
            assert!(
                sql.contains(expected),
                "op {op}: expected {expected:?} in {sql:?}"
            );
        }
    }

    #[test]
    fn smart_rules_contains_escapes_like_wildcards() {
        let rules = SmartRules {
            match_all: true,
            rules: vec![rule("album", "contains", "50%_off")],
        };
        let (sql, params) = smart_playlist_where(&rules).unwrap();
        assert!(sql.contains("album LIKE ? ESCAPE '\\'"));
        assert_eq!(params[0], Value::Text("%50\\%\\_off%".into()));
    }

    #[test]
    fn smart_rules_contains_on_numeric_uses_cast() {
        let rules = SmartRules {
            match_all: true,
            rules: vec![rule("year", "contains", "200")],
        };
        let (sql, params) = smart_playlist_where(&rules).unwrap();
        assert!(sql.contains("CAST(year AS TEXT) LIKE ? ESCAPE '\\'"));
        assert_eq!(params[0], Value::Text("%200%".into()));
    }

    #[test]
    fn smart_rules_contains_escapes_backslashes() {
        let rules = SmartRules {
            match_all: true,
            rules: vec![rule("album", "contains", "a\\b")],
        };
        let (_, params) = smart_playlist_where(&rules).unwrap();
        // escape_like turns '\' into '\\' inside the LIKE pattern.
        assert_eq!(params[0], Value::Text("%a\\\\b%".into()));
    }

    #[test]
    fn smart_rules_coerce_favorite_values() {
        for raw in ["true", "1", "yes", "TRUE", " Yes "] {
            let rules = SmartRules {
                match_all: true,
                rules: vec![rule("favorite", "eq", raw)],
            };
            let (_, params) = smart_playlist_where(&rules).unwrap();
            assert_eq!(params[0], Value::Integer(1), "{raw:?} should be truthy");
        }
        for raw in ["false", "0", "no", "FALSE", " No "] {
            let rules = SmartRules {
                match_all: true,
                rules: vec![rule("favorite", "eq", raw)],
            };
            let (_, params) = smart_playlist_where(&rules).unwrap();
            assert_eq!(params[0], Value::Integer(0), "{raw:?} should be falsy");
        }
    }

    #[test]
    fn smart_rules_coerce_unparseable_numbers_to_zero() {
        let rules = SmartRules {
            match_all: true,
            rules: vec![rule("year", "eq", "not-a-year")],
        };
        let (_, params) = smart_playlist_where(&rules).unwrap();
        assert_eq!(params[0], Value::Integer(0));
    }

    #[test]
    fn smart_rules_text_fields_keep_string_values() {
        let rules = SmartRules {
            match_all: true,
            rules: vec![rule("genre", "eq", "Jazz"), rule("artist", "eq", "Miles")],
        };
        let (_, params) = smart_playlist_where(&rules).unwrap();
        assert_eq!(params[0], Value::Text("Jazz".into()));
        assert_eq!(params[1], Value::Text("Miles".into()));
    }

    #[test]
    fn smart_rules_reject_unknown_fields() {
        let rules = SmartRules {
            match_all: true,
            rules: vec![SmartRule {
                field: "nope; DROP TABLE tracks".into(),
                op: "eq".into(),
                value: "x".into(),
            }],
        };
        assert!(smart_playlist_where(&rules).is_err());
    }

    #[test]
    fn smart_rules_reject_unsupported_operators() {
        let rules = SmartRules {
            match_all: true,
            rules: vec![rule("genre", "regex", "Jazz")],
        };
        assert!(smart_playlist_where(&rules).is_err());
    }

    #[test]
    fn smart_rules_reject_empty_rule_sets() {
        let rules = SmartRules {
            match_all: true,
            rules: vec![],
        };
        assert!(smart_playlist_where(&rules).is_err());
    }

    #[test]
    fn validate_rules_accepts_valid_and_rejects_invalid() {
        let ok = SmartRules {
            match_all: true,
            rules: vec![rule("genre", "contains", "Jazz")],
        };
        assert!(validate_rules(&ok).is_ok());
        let bad = SmartRules {
            match_all: true,
            rules: vec![rule("unknown", "eq", "x")],
        };
        assert!(validate_rules(&bad).is_err());
        let empty = SmartRules {
            match_all: true,
            rules: vec![],
        };
        assert!(validate_rules(&empty).is_err());
    }

    #[test]
    fn settings_defaults_are_sane() {
        let s = Settings::default();
        assert_eq!(s.theme, "system");
        assert_eq!(s.repeat, RepeatMode::Off);
        assert_eq!(s.eq_gains.len(), 10);
    }
}
