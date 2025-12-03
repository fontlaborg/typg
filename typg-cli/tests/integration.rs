use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;
use tempfile::tempdir;

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
fn find_scripts_arab_outputs_expected_font() {
    let fonts = match fonts_dir() {
        Some(dir) => dir,
        None => return, // skip when fixtures are unavailable
    };

    let output = Command::new(env!("CARGO_BIN_EXE_typg"))
        .args(["find", "--scripts", "arab"])
        .arg(&fonts)
        .output()
        .expect("run typg");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 1, "stdout:\n{}", stdout);
    assert!(lines[0].ends_with("NotoNaskhArabic-Regular.ttf"));
}

#[test]
fn find_variable_json_respects_jobs_flag() {
    let fonts = match fonts_dir() {
        Some(dir) => dir,
        None => return, // skip when fixtures are unavailable
    };

    let output = Command::new(env!("CARGO_BIN_EXE_typg"))
        .args(["find", "--variable", "--json", "--jobs", "1"])
        .arg(&fonts)
        .output()
        .expect("run typg");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let parsed: Value = serde_json::from_str(&stdout).expect("parse json output");
    let arr = parsed.as_array().expect("find --json returns a JSON array");
    assert!(!arr.is_empty(), "expected at least one match");

    let paths: Vec<&str> = arr
        .iter()
        .filter_map(|entry| entry["source"]["path"].as_str())
        .collect();

    assert!(
        paths.iter().any(|p| p.ends_with("Kalnia[wdth,wght].ttf")),
        "variable search should include Kalnia"
    );
}

#[test]
fn find_paths_output_is_ansi_free_even_with_color_always() {
    let fonts = match fonts_dir() {
        Some(dir) => dir,
        None => return, // skip when fixtures are unavailable
    };

    let output = Command::new(env!("CARGO_BIN_EXE_typg"))
        .args(["find", "--scripts", "latn", "--paths", "--color", "always"])
        .arg(&fonts)
        .output()
        .expect("run typg");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.lines().count() > 0, "expected some paths in output");
    assert!(
        !stdout.contains("\u{1b}["),
        "paths output should not include ANSI codes even when color is forced"
    );
}

#[test]
fn find_name_regex_matches_family_name() {
    let fonts = match fonts_dir() {
        Some(dir) => dir,
        None => return, // skip when fixtures are unavailable
    };

    let output = Command::new(env!("CARGO_BIN_EXE_typg"))
        .args(["find", "--name", "Noto Sans", "--json"])
        .arg(&fonts)
        .output()
        .expect("run typg");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let parsed: Value = serde_json::from_str(&stdout).expect("parse json output");
    let arr = parsed.as_array().expect("find --json returns array");
    assert!(
        arr.iter().any(|entry| entry["source"]["path"]
            .as_str()
            .map(|p| p.ends_with("NotoSans-Regular.ttf"))
            .unwrap_or(false)),
        "name regex should match family name from the name table"
    );
}

#[test]
fn cache_add_find_and_clean_cycle() {
    let fonts = match fonts_dir() {
        Some(dir) => dir,
        None => return, // skip when fixtures are unavailable
    };

    let tmp = tempdir().expect("tempdir");
    let cache_path = tmp.path().join("cache.json");
    let mirror = tmp.path().join("fonts");
    fs::create_dir_all(&mirror).expect("mirror dir");

    for entry in fs::read_dir(&fonts).expect("read fixtures") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_file() {
            let dest = mirror.join(path.file_name().expect("filename"));
            fs::copy(&path, &dest).expect("copy font fixture");
        }
    }

    let add = Command::new(env!("CARGO_BIN_EXE_typg"))
        .args(["cache", "add", "--cache-path"])
        .arg(&cache_path)
        .arg(&mirror)
        .output()
        .expect("run cache add");
    assert!(
        add.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    assert!(cache_path.exists(), "cache file should be created");

    let list = Command::new(env!("CARGO_BIN_EXE_typg"))
        .args(["cache", "list", "--cache-path"])
        .arg(&cache_path)
        .arg("--json")
        .output()
        .expect("run cache list");
    assert!(
        list.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&list.stderr)
    );
    let listed: Value = serde_json::from_slice(&list.stdout).expect("parse list json");
    let initial_len = listed.as_array().map(|a| a.len()).unwrap_or(0);
    assert!(initial_len > 0, "cache should contain entries");

    let find = Command::new(env!("CARGO_BIN_EXE_typg"))
        .args(["cache", "find", "--cache-path"])
        .arg(&cache_path)
        .args(["--scripts", "latn", "--json"])
        .output()
        .expect("run cache find");
    assert!(
        find.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&find.stderr)
    );
    let found: Value = serde_json::from_slice(&find.stdout).expect("parse find json");
    let arr = found.as_array().expect("find returns array");
    assert!(
        arr.iter().any(|entry| entry["source"]["path"]
            .as_str()
            .map(|p| p.ends_with("NotoSans-Regular.ttf"))
            .unwrap_or(false)),
        "cached find should include NotoSans-Regular.ttf"
    );

    // Remove one font and ensure clean drops it.
    let removed = mirror.join("NotoSans-Regular.ttf");
    fs::remove_file(&removed).expect("remove a cached font");

    let clean = Command::new(env!("CARGO_BIN_EXE_typg"))
        .args(["cache", "clean", "--cache-path"])
        .arg(&cache_path)
        .output()
        .expect("run cache clean");
    assert!(
        clean.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&clean.stderr)
    );

    let list_after = Command::new(env!("CARGO_BIN_EXE_typg"))
        .args(["cache", "list", "--cache-path"])
        .arg(&cache_path)
        .arg("--json")
        .output()
        .expect("run cache list after clean");
    let listed_after: Value =
        serde_json::from_slice(&list_after.stdout).expect("parse list json after clean");
    let after_len = listed_after.as_array().map(|a| a.len()).unwrap_or(0);
    assert!(
        after_len < initial_len,
        "clean should prune missing entries ({} -> {})",
        initial_len,
        after_len
    );
}
