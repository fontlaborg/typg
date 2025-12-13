/// The neighborhood explorer who knows where all the fonts are hiding
///
/// Think of us as the friendly neighborhood font scout who goes door-to-door
/// finding every font that calls your filesystem home. We'll climb through
/// directory trees, peek into folder corners, and come back with a complete
/// census of all the typographic residents in your chosen neighborhoods.
///
/// Whether fonts are living openly in plain sight or hiding in nested
/// subdirectories, we'll find them. We're even brave enough to follow
/// symlinks if you give us permission - those mysterious pathways often
/// lead to the most interesting font discoveries.
///
/// Made with adventurous spirit at FontLab https://www.fontlab.com/

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use walkdir::WalkDir;

/// A business card for every font we meet on our explorations
///
/// When we find a font during our neighborhood adventures, we give it
/// this simple calling card that tells you exactly where it lives. No
/// frills, no fuss - just the perfect address for later visits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypgFontSourceRef {
    /// The exact street address where this font hangs out
    pub path: PathBuf,
}

/// The explorer's contract - how we promise to find fonts for you
/// 
/// Any font discovery service must be brave enough to venture into
/// the unknown and return with tales of all the fonts they met.
/// Whether they're crawling filesystems, consulting databases, or
/// reading cache indices, they all come back with the same treasure:
/// a list of fonts waiting to be discovered.
pub trait FontDiscovery {
    /// Sets out on expedition and returns with all the fonts found
    fn discover(&self) -> Result<Vec<TypgFontSourceRef>>;
}

/// The brave filesystem explorer who never says no to an adventure
/// 
/// We're the expedition leaders who'll climb any directory tree,
/// cross any symlink bridge, and search every nook and cranny for
/// typographic treasures. Give us a list of neighborhoods to explore
/// and we'll come back with a complete census of every font that
/// calls those places home.
#[derive(Debug, Clone)]
pub struct PathDiscovery {
    /// The starting points for our font-finding expeditions
    roots: Vec<PathBuf>,
    /// Should we be brave enough to follow those mysterious symlink shortcuts?
    follow_symlinks: bool,
}

impl PathDiscovery {
    /// Assembles our expedition team and maps out our adventure route
    /// 
    /// Give us your list of neighborhoods to explore and we'll prepare
    /// our expedition kit. By default, we play it safe and stick to the
    /// beaten path - no mysterious symlink shortcuts unless you say so.
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

    /// Decides whether we're brave enough to follow mysterious shortcuts
    /// 
    /// Symlinks are like teleportation portals in the filesystem - they
    /// can lead to wondrous discoveries or endless loops. We'll follow them
    /// if you're feeling adventurous, but we're happy to stay on solid ground
    /// if you prefer the conservative approach.
    pub fn follow_symlinks(mut self, follow: bool) -> Self {
        self.follow_symlinks = follow;
        self
    }
}

impl FontDiscovery for PathDiscovery {
    /// Sets out on our grand font-finding expedition through the filesystem jungle
    /// 
    /// We'll visit every neighborhood on our map, climb directory trees with
    /// the agility of a seasoned explorer, and carefully examine every file
    /// we encounter. Only the true typographic treasures get added to our
    /// collection - we're discerning explorers who know quality when we see it.
    /// 
    /// Returns: A complete catalog of every font we discovered on our adventure.
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

/// The expert detective who can spot a font from just its file extension
/// 
/// We've seen thousands of fonts in our day, and we've learned to
/// recognize them by their distinctive signatures. TTF, OTF, TTC, OTC -
/// we know them all. Case doesn't matter to us - we're equal-opportunity
/// font identifiers who believe every font deserves to be discovered.
/// 
/// Returns true if this extension belongs to a legitimate format.
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
