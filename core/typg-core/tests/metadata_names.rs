use std::env;
use std::path::PathBuf;

use regex::Regex;
use typg_core::query::Query;
use typg_core::search::{search, SearchOptions};

fn fonts_dir() -> Option<PathBuf> {
    if let Ok(env_override) = env::var("TYPF_TEST_FONTS") {
        let path = PathBuf::from(env_override);
        if let Ok(dir) = path.canonicalize() {
            return Some(dir);
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest_dir
            .join("..")
            .join("..")
            .join("typf")
            .join("test-fonts"),
        manifest_dir
            .join("..")
            .join("linked")
            .join("typf")
            .join("test-fonts"),
        manifest_dir.join("..").join("..").join("test-fonts"),
    ];

    for candidate in candidates {
        if let Ok(dir) = candidate.canonicalize() {
            return Some(dir);
        }
    }

    None
}

#[test]
fn name_queries_use_name_table_strings() {
    let fonts = match fonts_dir() {
        Some(dir) => dir,
        None => return, // skip when fixtures are unavailable
    };

    let query = Query::new().with_name_patterns(vec![Regex::new("Noto Sans").unwrap()]);
    let matches = search(&[fonts], &query, &SearchOptions::default()).expect("search fonts");

    assert!(
        matches.iter().any(|m| m
            .source
            .path
            .as_path()
            .file_name()
            .map(|f| f.to_string_lossy().ends_with("NotoSans-Regular.ttf"))
            .unwrap_or(false)),
        "expected NotoSans-Regular.ttf to match name-table regex"
    );
}
