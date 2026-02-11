/// Testing our font-finding expedition skills
///
/// These tests make sure our brave filesystem explorers can find fonts
/// in all the usual hiding spots - nested directories, various file
/// extensions, and even those tricky symlink shortcuts when we're feeling
/// adventurous. No font left behind!
use std::path::PathBuf;

use typg_core::discovery::{FontDiscovery, PathDiscovery};

#[test]
fn discovers_common_font_extensions_recursively() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();

    let font1 = root.join("a.ttf");
    let font2_dir = root.join("nested");
    std::fs::create_dir_all(&font2_dir).unwrap();
    let font2 = font2_dir.join("b.otf");

    std::fs::write(&font1, b"\0\0font1").unwrap();
    std::fs::write(&font2, b"\0\0font2").unwrap();

    let discovery = PathDiscovery::new([PathBuf::from(root)]);
    let fonts = discovery.discover().expect("discover");

    let paths: Vec<PathBuf> = fonts.into_iter().map(|f| f.path).collect();
    assert!(paths.contains(&font1));
    assert!(paths.contains(&font2));
}

#[test]
fn ignores_non_font_extensions() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    std::fs::write(root.join("readme.txt"), b"hello").unwrap();

    let discovery = PathDiscovery::new([root.to_path_buf()]);
    let fonts = discovery.discover().expect("discover");

    assert!(fonts.is_empty());
}

#[test]
fn returns_error_for_missing_root() {
    let missing = PathBuf::from("/nonexistent/typg-fonts");
    let discovery = PathDiscovery::new([missing]);
    let result = discovery.discover();

    assert!(result.is_err());
}
