/// Filesystem font discovery — the first step in every search.
///
/// Before you can query font metadata, you need to know where the fonts live.
/// This module walks directory trees, identifies font files by extension, and
/// hands back a list of paths for the search engine to open.
///
/// Recognized extensions: `.ttf` (TrueType), `.otf` (OpenType/CFF),
/// `.ttc` (TrueType Collection), `.otc` (OpenType Collection).
/// WOFF/WOFF2 web fonts are not included — they're compressed containers
/// meant for browsers, not typically installed on the system.
///
/// Directories that can't be read (permissions, broken mounts, dangling
/// symlinks) are silently skipped. The walk continues. A single locked
/// folder shouldn't kill a search across thousands of fonts.
///
/// Made by FontLab <https://www.fontlab.com/>
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use walkdir::WalkDir;

/// A font file found on disk during discovery.
///
/// At this stage we only know *where* the file is, not what's inside it.
/// Metadata extraction happens later in the [`search`](crate::search) module.
/// A TTC/OTC collection file appears as a single `TypgFontSourceRef` here;
/// the search module will enumerate individual faces within it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypgFontSourceRef {
    /// Absolute (or as-given) path to the font file on disk.
    pub path: PathBuf,
}

/// Trait for font discovery backends.
///
/// The default implementation ([`PathDiscovery`]) walks the local filesystem.
/// Alternative backends could scan network shares, font servers, or package
/// managers — anything that can produce a list of font file paths.
pub trait FontDiscovery {
    /// Scan for font files and return their locations.
    ///
    /// Implementations should be resilient: skip inaccessible paths rather
    /// than aborting the entire scan. Return `Err` only for truly fatal
    /// problems (e.g., the root path itself doesn't exist).
    fn discover(&self) -> Result<Vec<TypgFontSourceRef>>;
}

/// Discovers fonts by walking filesystem paths.
///
/// Give it one or more root directories. It recurses into every subdirectory,
/// checks each file's extension, and collects anything that looks like a font.
///
/// Symlink behavior matters in practice: macOS `/System/Library/Fonts` contains
/// symlinks into sealed system volumes, and many Linux setups symlink font
/// directories across partitions. Enable [`follow_symlinks`](Self::follow_symlinks)
/// when you want to reach fonts behind those links. Leave it off (the default)
/// to avoid infinite loops from circular symlinks.
///
/// # Example
///
/// ```rust,no_run
/// use typg_core::discovery::{PathDiscovery, FontDiscovery};
///
/// let fonts = PathDiscovery::new(["/usr/share/fonts", "/home/me/.fonts"])
///     .follow_symlinks(true)
///     .discover()?;
///
/// println!("Found {} font files", fonts.len());
/// # Ok::<(), anyhow::Error>(())
/// ```
#[derive(Debug, Clone)]
pub struct PathDiscovery {
    /// Root directories to walk. Each is traversed recursively.
    roots: Vec<PathBuf>,
    /// Follow symbolic links during traversal. Off by default to prevent
    /// infinite loops from circular symlinks.
    follow_symlinks: bool,
}

impl PathDiscovery {
    /// Create a discovery instance for the given root paths.
    ///
    /// Each path should be a directory. If you pass a file path, `walkdir`
    /// will yield just that one file (which is fine for single-file checks).
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
    /// Walk all root paths and return every font file found.
    ///
    /// The walk is resilient: directories that can't be read (permission
    /// denied, broken symlinks, vanished network mounts) are silently
    /// skipped. One unreadable folder won't abort a scan of thousands.
    ///
    /// Returns `Err` only if a root path itself doesn't exist — that's
    /// likely a typo, and the caller should know about it.
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

/// Check whether a file has a recognized font extension.
///
/// Recognized: `.ttf` (TrueType), `.otf` (OpenType/CFF), `.ttc` and `.otc`
/// (collection files that bundle multiple faces in one file).
/// Case-insensitive — `ARIAL.TTF` and `arial.ttf` both match.
///
/// Not recognized: `.woff`, `.woff2` (web font containers), `.dfont`
/// (legacy macOS resource-fork format), `.fon` (Windows bitmap fonts).
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
