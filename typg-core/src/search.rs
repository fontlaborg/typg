/// The gentle art of font discovery and conversation
///
/// Like a curious detective who never stops asking questions, this module
/// extracts secrets hidden inside font files. We listen carefully to what
/// each font has to say, then help you find the ones that are singing your tune.
///
/// Made with care at FontLab https://www.fontlab.com/

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;
use read_fonts::tables::name::NameId;
use read_fonts::types::Tag;
use read_fonts::{FontRef, TableProvider};
use serde::{Deserialize, Serialize};
use skrifa::{FontRef as SkrifaFontRef, MetadataProvider};

use crate::discovery::{FontDiscovery, PathDiscovery};
use crate::query::Query;
use crate::tags::{tag4, tag_to_string};

/// Every font's personal biography in convenient story form
///
/// We gather all the delightful details that make each font unique -
/// their names, talents, family connections, and secret abilities.
/// Think of this as the font's dating profile, showing what makes them
/// special and what conversations they enjoy having.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypgFontFaceMeta {
    /// All the names this font goes by - family, full, postscript, and nicknames
    pub names: Vec<String>,
    /// The dance moves this variable font can perform (weight, width, optical size...)
    #[serde(
        serialize_with = "serialize_tags",
        deserialize_with = "deserialize_tags"
    )]
    pub axis_tags: Vec<Tag>,
    /// Special typographic tricks and typographic talents tucked away inside
    #[serde(
        serialize_with = "serialize_tags",
        deserialize_with = "deserialize_tags"
    )]
    pub feature_tags: Vec<Tag>,
    /// Languages and scripts this font can speak fluently
    #[serde(
        serialize_with = "serialize_tags",
        deserialize_with = "deserialize_tags"
    )]
    pub script_tags: Vec<Tag>,
    /// The building blocks available in this font's toolkit
    #[serde(
        serialize_with = "serialize_tags",
        deserialize_with = "deserialize_tags"
    )]
    pub table_tags: Vec<Tag>,
    /// Every character this font knows how to draw - their complete vocabulary
    pub codepoints: Vec<char>,
    /// Can this font change shape like a chameleon, or stay true to one form?
    pub is_variable: bool,
    /// How bold does this font think it is? (100-900 typographic scale)
    #[serde(default)]
    pub weight_class: Option<u16>,
    /// How wide does this font like to stretch? (1-9 condensed to expanded)
    #[serde(default)]
    pub width_class: Option<u16>,
    /// What typographic family does this font belong to? (class and subgroup)
    #[serde(default)]
    pub family_class: Option<(u8, u8)>,
}

/// Where each font calls home and how to find them at the party
///
/// Some fonts live alone in their own apartment (TTF/OTF files),
/// while others share a house with roommates (TTC/OTC collections).
/// We keep track of both the address and which door to knock on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypgFontSource {
    /// The street address where this font lives on your filesystem
    pub path: PathBuf,
    /// Which door in the font collection apartment complex to knock on
    pub ttc_index: Option<u32>,
}

impl TypgFontSource {
    /// Creates a friendly address that includes apartment numbers for collections
    /// 
    /// Regular fonts get their regular address, but fonts in collections
    /// get a helpful "#0", "#1", etc. suffix to show which roommate we mean.
    pub fn path_with_index(&self) -> String {
        if let Some(idx) = self.ttc_index {
            format!("{}#{idx}", self.path.display())
        } else {
            self.path.display().to_string()
        }
    }
}

/// The perfect matchmaker pairing: who they are and where to find them
///
/// We bring together the font's personal story (metadata) with their
/// actual living situation (source). It's like handing someone a
/// dating profile along with directions to the coffee shop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypgFontFaceMatch {
    /// Where this font hangs out and how to visit them
    pub source: TypgFontSource,
    /// All the juicy details about what makes this font special
    pub metadata: TypgFontFaceMeta,
}

/// How we like to conduct our search expeditions
///
/// These options let you fine-tune your font-finding adventure.
/// Want to follow mysterious pathways (symlinks)? Need more hands
/// on deck for a big search expedition? We've got you covered.
#[derive(Debug, Default, Clone)]
pub struct SearchOptions {
    /// Should we follow those mysterious shortcut pathways that symlinks create?
    pub follow_symlinks: bool,
    /// How many search elves should we hire for this expedition? (None = let the system decide)
    pub jobs: Option<usize>,
}

/// The grand orchestrator of font discovery expeditions
/// 
/// We take your list of neighborhoods to explore (paths), your specific
/// criteria for the perfect font match (query), and your preferred style
/// of exploration (opts). Then we venture forth, chat up all the fonts
/// we meet, and return with the ones that caught your eye.
/// 
/// Returns: A tastefully arranged collection of font matches, sorted by neighborhood.
pub fn search(
    paths: &[PathBuf],
    query: &Query,
    opts: &SearchOptions,
) -> Result<Vec<TypgFontFaceMatch>> {
    let discovery = PathDiscovery::new(paths.iter().cloned()).follow_symlinks(opts.follow_symlinks);
    let candidates = discovery.discover()?;

    let run_search = || -> Result<Vec<TypgFontFaceMatch>> {
        let metadata: Result<Vec<Vec<TypgFontFaceMatch>>> = candidates
            .par_iter()
            .map(|loc| load_metadata(&loc.path))
            .collect();

        let mut matches: Vec<TypgFontFaceMatch> = metadata?
            .into_par_iter()
            .flatten()
            .filter(|face| query.matches(&face.metadata))
            .collect();

        sort_matches(&mut matches);
        Ok(matches)
    };

    if let Some(jobs) = opts.jobs {
        let pool = ThreadPoolBuilder::new().num_threads(jobs).build()?;
        pool.install(run_search)
    } else {
        run_search()
    }
}

/// Speed dating with fonts you've already met (no file system required)
/// 
/// When you have a list of fonts you've already gotten to know, sometimes
/// you just want to filter them by new criteria without re-reading all those
/// font files. This is like having address cards for everyone at the party
/// and quickly finding who matches your new interests.
/// 
/// Returns: A curated subset of your existing font acquaintances.
pub fn filter_cached(entries: &[TypgFontFaceMatch], query: &Query) -> Vec<TypgFontFaceMatch> {
    let mut matches: Vec<TypgFontFaceMatch> = entries
        .iter()
        .filter(|entry| query.matches(&entry.metadata))
        .cloned()
        .collect();

    sort_matches(&mut matches);
    matches
}

/// The gentle interrogation of a font file to learn all its secrets
/// 
/// We knock on the font's door, politely ask to come in, and then
/// carefully extract every interesting detail about what makes it
/// special. Like a good interviewer, we know exactly which questions
/// to ask to get the font to open up and share its story.
/// 
/// For font collections, we chat with each roommate individually.
fn load_metadata(path: &Path) -> Result<Vec<TypgFontFaceMatch>> {
    let data = fs::read(path).with_context(|| format!("reading font {}", path.display()))?;
    let mut metas = Vec::new();

    for font in FontRef::fonts(&data) {
        let font = font?;
        let ttc_index = font.ttc_index();
        let sfont = if let Some(idx) = ttc_index {
            SkrifaFontRef::from_index(&data, idx)?
        } else {
            SkrifaFontRef::new(&data)?
        };

        let names = collect_names(&font);
        let mut axis_tags = collect_axes(&font);
        let mut feature_tags = collect_features(&font);
        let mut script_tags = collect_scripts(&font);
        let mut table_tags = collect_tables(&font);
        let mut codepoints = collect_codepoints(&sfont);
        let fvar_tag = Tag::new(b"fvar");
        let is_variable = table_tags.contains(&fvar_tag);
        let (weight_class, width_class, family_class) = collect_classification(&font);

        dedup_tags(&mut axis_tags);
        dedup_tags(&mut feature_tags);
        dedup_tags(&mut script_tags);
        dedup_tags(&mut table_tags);
        dedup_codepoints(&mut codepoints);

        metas.push(TypgFontFaceMatch {
            source: TypgFontSource {
                path: path.to_path_buf(),
                ttc_index,
            },
            metadata: TypgFontFaceMeta {
                names: dedup_names(names, path),
                axis_tags,
                feature_tags,
                script_tags,
                table_tags,
                codepoints,
                is_variable,
                weight_class,
                width_class,
                family_class,
            },
        });
    }

    Ok(metas)
}

fn collect_tables(font: &FontRef) -> Vec<Tag> {
    font.table_directory
        .table_records()
        .iter()
        .map(|rec| rec.tag())
        .collect()
}

fn collect_axes(font: &FontRef) -> Vec<Tag> {
    if let Ok(fvar) = font.fvar() {
        if let Ok(axes) = fvar.axes() {
            return axes.iter().map(|axis| axis.axis_tag()).collect();
        }
    }
    Vec::new()
}

fn collect_features(font: &FontRef) -> Vec<Tag> {
    let mut tags = Vec::new();
    if let Ok(gsub) = font.gsub() {
        if let Ok(list) = gsub.feature_list() {
            tags.extend(list.feature_records().iter().map(|rec| rec.feature_tag()));
        }
    }
    if let Ok(gpos) = font.gpos() {
        if let Ok(list) = gpos.feature_list() {
            tags.extend(list.feature_records().iter().map(|rec| rec.feature_tag()));
        }
    }
    tags
}

fn collect_scripts(font: &FontRef) -> Vec<Tag> {
    let mut tags = Vec::new();
    if let Ok(gsub) = font.gsub() {
        if let Ok(list) = gsub.script_list() {
            tags.extend(list.script_records().iter().map(|rec| rec.script_tag()));
        }
    }
    if let Ok(gpos) = font.gpos() {
        if let Ok(list) = gpos.script_list() {
            tags.extend(list.script_records().iter().map(|rec| rec.script_tag()));
        }
    }
    tags
}

fn collect_codepoints(font: &SkrifaFontRef) -> Vec<char> {
    let mut cps = Vec::new();
    for (cp, _) in font.charmap().mappings() {
        if let Some(ch) = char::from_u32(cp) {
            cps.push(ch);
        }
    }
    cps
}

fn collect_names(font: &FontRef) -> Vec<String> {
    let mut names = Vec::new();

    if let Ok(name_table) = font.name() {
        let data = name_table.string_data();
        let wanted = [
            NameId::FAMILY_NAME,
            NameId::TYPOGRAPHIC_FAMILY_NAME,
            NameId::SUBFAMILY_NAME,
            NameId::TYPOGRAPHIC_SUBFAMILY_NAME,
            NameId::FULL_NAME,
            NameId::POSTSCRIPT_NAME,
        ];

        for record in name_table.name_record() {
            if !record.is_unicode() {
                continue;
            }
            if !wanted.contains(&record.name_id()) {
                continue;
            }
            if let Ok(entry) = record.string(data) {
                let rendered = entry.to_string();
                if !rendered.trim().is_empty() {
                    names.push(rendered);
                }
            }
        }
    }

    names
}

fn collect_classification(font: &FontRef) -> (Option<u16>, Option<u16>, Option<(u8, u8)>) {
    match font.os2() {
        Ok(table) => {
            let raw_family = table.s_family_class() as u16;
            let class = (raw_family >> 8) as u8;
            let subclass = (raw_family & 0x00FF) as u8;
            (
                Some(table.us_weight_class()),
                Some(table.us_width_class()),
                Some((class, subclass)),
            )
        }
        Err(_) => (None, None, None),
    }
}

fn sort_matches(matches: &mut [TypgFontFaceMatch]) {
    matches.sort_by(|a, b| {
        a.source
            .path
            .cmp(&b.source.path)
            .then_with(|| a.source.ttc_index.cmp(&b.source.ttc_index))
    });
}

fn dedup_tags(tags: &mut Vec<Tag>) {
    tags.sort_unstable();
    tags.dedup();
}

fn dedup_codepoints(codepoints: &mut Vec<char>) {
    codepoints.sort_unstable();
    codepoints.dedup();
}

fn dedup_names(mut names: Vec<String>, path: &Path) -> Vec<String> {
    names.push(
        path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string()),
    );

    for name in names.iter_mut() {
        *name = name.trim().to_string();
    }

    names.retain(|n| !n.is_empty());
    names.sort_unstable();
    names.dedup();
    names
}

fn serialize_tags<S>(tags: &[Tag], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let as_strings: Vec<String> = tags.iter().copied().map(tag_to_string).collect();
    as_strings.serialize(serializer)
}

fn deserialize_tags<'de, D>(deserializer: D) -> Result<Vec<Tag>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: Vec<String> = Vec::<String>::deserialize(deserializer)?;
    raw.into_iter()
        .map(|s| tag4(&s).map_err(serde::de::Error::custom))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_names_adds_fallback_and_trims() {
        let names = vec!["  Alpha  ".to_string(), "Alpha".to_string()];
        let path = Path::new("/fonts/Beta.ttf");
        let deduped = dedup_names(names, path);

        assert!(
            deduped.contains(&"Alpha".to_string()),
            "original names should be trimmed and kept"
        );
        assert!(
            deduped.contains(&"Beta".to_string()),
            "file stem should be added as fallback name"
        );
        assert_eq!(
            deduped.len(),
            2,
            "dedup should remove duplicate entries and empty strings"
        );
    }

    #[test]
    fn dedup_tags_sorts_and_dedups() {
        let mut tags = vec![
            tag4("wght").unwrap(),
            tag4("wght").unwrap(),
            tag4("GSUB").unwrap(),
        ];
        dedup_tags(&mut tags);

        assert_eq!(tags, vec![tag4("GSUB").unwrap(), tag4("wght").unwrap()]);
    }

    #[test]
    fn dedup_codepoints_sorts_and_dedups() {
        let mut cps = vec!['b', 'a', 'b'];
        dedup_codepoints(&mut cps);
        assert_eq!(cps, vec!['a', 'b']);
    }
}
