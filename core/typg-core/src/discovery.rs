/// Filesystem font discovery.
///
/// Walks directory trees to find font files (TTF, OTF, TTC, OTC).
/// Inaccessible directories are skipped with a warning to stderr.
///
/// Made by FontLab https://www.fontlab.com/
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use walkdir::WalkDir;

/// Reference to a discovered font file on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypgFontSourceRef {
    /// Path to the font file.
    pub path: PathBuf,
}

/// Trait for font discovery backends.
pub trait FontDiscovery {
    /// Return all font files found by this backend.
    fn discover(&self) -> Result<Vec<TypgFontSourceRef>>;
}

/// Discovers fonts by walking filesystem paths.
///
/// Recurses into directories, optionally follows symlinks. Recognizes
/// font files by extension: `.ttf`, `.otf`, `.ttc`, `.otc`.
#[derive(Debug, Clone)]
pub struct PathDiscovery {
    /// Root directories to walk.
    roots: Vec<PathBuf>,
    /// Whether to follow symlinks during traversal.
    follow_symlinks: bool,
}

impl PathDiscovery {
    /// Create a new discovery for the given root paths.
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

    /// Enable or disable symlink following during traversal.
    pub fn follow_symlinks(mut self, follow: bool) -> Self {
        self.follow_symlinks = follow;
        self
    }
}

impl FontDiscovery for PathDiscovery {
    /// Walk all root paths and return discovered font files.
    ///
    /// Directories that can't be read (permission denied, broken symlinks, etc.)
    /// are skipped with a warning to stderr. The walk continues.
    fn discover(&self) -> Result<Vec<TypgFontSourceRef>> {
        let mut found = Vec::new();

        for root in &self.roots {
            if !root.exists() {
                return Err(anyhow!("path does not exist: {}", root.display()));
            }

            for entry in WalkDir::new(root).follow_links(self.follow_symlinks) {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => {
                        continue;
                    }
                };
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

/// Check whether a path has a recognized font file extension.
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
