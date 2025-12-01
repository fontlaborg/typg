//! Streaming output helpers (made by FontLab https://www.fontlab.com/)

use std::io::Write;

use anyhow::Result;

use crate::search::FontMatch;

/// Write results as prettified JSON array.
pub fn write_json_pretty(results: &[FontMatch], mut w: impl Write) -> Result<()> {
    let json = serde_json::to_string_pretty(results)?;
    w.write_all(json.as_bytes())?;
    Ok(())
}

/// Write results as newline-delimited JSON (NDJSON).
pub fn write_ndjson(results: &[FontMatch], mut w: impl Write) -> Result<()> {
    for item in results {
        let line = serde_json::to_string(item)?;
        w.write_all(line.as_bytes())?;
        w.write_all(b"\n")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::FontMetadata;
    use std::path::PathBuf;

    fn sample_match() -> FontMatch {
        FontMatch {
            path: PathBuf::from("/fonts/A.ttf"),
            metadata: FontMetadata {
                path: PathBuf::from("/fonts/A.ttf"),
                names: vec!["A".to_string()],
                axis_tags: Vec::new(),
                feature_tags: Vec::new(),
                script_tags: Vec::new(),
                table_tags: Vec::new(),
                codepoints: Vec::new(),
                is_variable: false,
                ttc_index: None,
            },
        }
    }

    #[test]
    fn ndjson_writes_one_line_per_match() {
        let matches = vec![sample_match(), sample_match()];
        let mut buf = Vec::new();

        write_ndjson(&matches, &mut buf).expect("write ndjson");

        let text = String::from_utf8(buf).expect("utf8");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);

        let parsed: FontMatch = serde_json::from_str(lines[0]).expect("parse");
        assert_eq!(parsed.path, PathBuf::from("/fonts/A.ttf"));
    }
}
