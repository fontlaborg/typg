//! Font discovery helpers for typg-core (made by FontLab https://www.fontlab.com/)

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use walkdir::WalkDir;

/// Path to a candidate font file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypgFontSourceRef {
    pub path: PathBuf,
}

/// Trait for enumerating fonts from some backing store (filesystem, cache index, etc.).
pub trait FontDiscovery {
    fn discover(&self) -> Result<Vec<TypgFontSourceRef>>;
}

/// Recursive filesystem walker that collects common font formats.
#[derive(Debug, Clone)]
pub struct PathDiscovery {
    roots: Vec<PathBuf>,
    follow_symlinks: bool,
}

impl PathDiscovery {
    pub fn new<I, P>(roots: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        let roots = roots.into_iter().map(Into::into).collect();
        Self {
            roots,
            follow_symlinks: false,
        }
    }

    pub fn follow_symlinks(mut self, follow: bool) -> Self {
        self.follow_symlinks = follow;
        self
    }
}

impl FontDiscovery for PathDiscovery {
    fn discover(&self) -> Result<Vec<TypgFontSourceRef>> {
        let mut found = Vec::new();

        for root in &self.roots {
            if !root.exists() {
                return Err(anyhow!("root path does not exist: {}", root.display()));
            }

            for entry in WalkDir::new(root).follow_links(self.follow_symlinks) {
                let entry = entry?;
                if entry.file_type().is_file() && is_font(entry.path()) {
                    found.push(TypgFontSourceRef {
                        path: entry.path().to_path_buf(),
                    });
                }
            }
        }

        Ok(found)
    }
}

fn is_font(path: &Path) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => ext.to_ascii_lowercase(),
        None => return false,
    };

    matches!(ext.as_str(), "ttf" | "otf" | "ttc" | "otc")
}

#[cfg(test)]
mod tests {
    use super::is_font;
    use super::FontDiscovery;
    use super::PathDiscovery;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn recognises_font_extensions() {
        assert!(is_font("/A/B/font.ttf".as_ref()));
        assert!(is_font("/A/B/font.OTF".as_ref()));
        assert!(!is_font("/A/B/font.txt".as_ref()));
        assert!(!is_font("/A/B/font".as_ref()));
    }

    #[test]
    fn discovers_nested_fonts() {
        let tmp = tempdir().expect("tempdir");
        let nested = tmp.path().join("a/b");
        fs::create_dir_all(&nested).expect("mkdir");
        let font_path = nested.join("sample.ttf");
        fs::write(&font_path, b"").expect("touch font");

        let discovery = PathDiscovery::new([tmp.path()]);
        let fonts = discovery.discover().expect("discover");

        assert!(fonts.iter().any(|f| f.path == font_path));
    }

    #[cfg(unix)]
    #[test]
    fn follows_symlinks_when_enabled() {
        use std::os::unix::fs::symlink;

        let tmp = tempdir().expect("tempdir");
        let real_dir = tmp.path().join("real");
        let link_dir = tmp.path().join("link");
        fs::create_dir_all(&real_dir).expect("mkdir real");
        let font_path = real_dir.join("linked.otf");
        fs::write(&font_path, b"").expect("touch font");
        symlink(&real_dir, &link_dir).expect("symlink");

        let discovery = PathDiscovery::new([&link_dir]).follow_symlinks(true);
        let fonts = discovery.discover().expect("discover");

        assert!(fonts.iter().any(|f| f.path.ends_with("linked.otf")));
    }
}
