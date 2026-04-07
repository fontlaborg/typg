/// Font metadata extraction and search.
///
/// Reads font files via `read-fonts` and `skrifa`, extracts metadata (names,
/// axes, features, scripts, tables, codepoints, OS/2 fields), and filters
/// results against a [`Query`]. Uses `rayon` for parallel processing.
///
/// Unreadable or unparseable files are skipped with a warning to stderr.
///
/// Made by FontLab https://www.fontlab.com/
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

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

/// Extracted metadata for a single font face.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypgFontFaceMeta {
    /// Name strings: family, full, postscript, subfamily, plus file stem as fallback.
    pub names: Vec<String>,
    /// Variation axis tags (wght, wdth, opsz, ...). Empty for static fonts.
    #[serde(
        serialize_with = "serialize_tags",
        deserialize_with = "deserialize_tags"
    )]
    pub axis_tags: Vec<Tag>,
    /// OpenType feature tags from GSUB and GPOS tables.
    #[serde(
        serialize_with = "serialize_tags",
        deserialize_with = "deserialize_tags"
    )]
    pub feature_tags: Vec<Tag>,
    /// Script tags from GSUB and GPOS tables.
    #[serde(
        serialize_with = "serialize_tags",
        deserialize_with = "deserialize_tags"
    )]
    pub script_tags: Vec<Tag>,
    /// Top-level table tags present in the font.
    #[serde(
        serialize_with = "serialize_tags",
        deserialize_with = "deserialize_tags"
    )]
    pub table_tags: Vec<Tag>,
    /// Unicode codepoints covered by the font's cmap.
    pub codepoints: Vec<char>,
    /// True if the font contains an `fvar` table (variable font).
    pub is_variable: bool,
    /// OS/2 usWeightClass (typically 100-900).
    #[serde(default)]
    pub weight_class: Option<u16>,
    /// OS/2 usWidthClass (1-9).
    #[serde(default)]
    pub width_class: Option<u16>,
    /// OS/2 sFamilyClass split into (class, subclass).
    #[serde(default)]
    pub family_class: Option<(u8, u8)>,
    /// Creator-related name strings (copyright, trademark, manufacturer, designer, description, vendor URL, designer URL, license, license URL).
    #[serde(default)]
    pub creator_names: Vec<String>,
    /// License-related name strings (copyright, license description, license URL).
    #[serde(default)]
    pub license_names: Vec<String>,
}

/// Location of a font face on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypgFontSource {
    /// Path to the font file.
    pub path: PathBuf,
    /// Index within a TTC/OTC collection, or `None` for single-face files.
    pub ttc_index: Option<u32>,
}

impl TypgFontSource {
    /// Format as `path#index` for collection members, plain path otherwise.
    pub fn path_with_index(&self) -> String {
        if let Some(idx) = self.ttc_index {
            format!("{}#{idx}", self.path.display())
        } else {
            self.path.display().to_string()
        }
    }
}

/// A search result: font metadata paired with its file location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypgFontFaceMatch {
    /// File location and collection index.
    pub source: TypgFontSource,
    /// Extracted metadata for this face.
    pub metadata: TypgFontFaceMeta,
}

/// Controls search parallelism and traversal behavior.
#[derive(Debug, Default, Clone)]
pub struct SearchOptions {
    /// Follow symlinks during directory traversal.
    pub follow_symlinks: bool,
    /// Worker thread count. `None` uses the rayon default (CPU count).
    pub jobs: Option<usize>,
}

/// Search filesystem paths for fonts matching a query. Returns all results sorted by path.
///
/// Files that can't be read or parsed are skipped with a warning to stderr.
pub fn search(
    paths: &[PathBuf],
    query: &Query,
    opts: &SearchOptions,
) -> Result<Vec<TypgFontFaceMatch>> {
    let discovery = PathDiscovery::new(paths.iter().cloned()).follow_symlinks(opts.follow_symlinks);
    let candidates = discovery.discover()?;

    let run_search = || -> Vec<TypgFontFaceMatch> {
        let mut matches: Vec<TypgFontFaceMatch> = candidates
            .par_iter()
            .flat_map_iter(|loc| match load_metadata(&loc.path) {
                Ok(faces) => faces,
                Err(e) => {
                    eprintln!("warning: {e}");
                    Vec::new()
                }
            })
            .filter(|face| query.matches(&face.metadata))
            .collect();

        sort_matches(&mut matches);
        matches
    };

    let matches = if let Some(jobs) = opts.jobs {
        let pool = ThreadPoolBuilder::new().num_threads(jobs).build()?;
        pool.install(run_search)
    } else {
        run_search()
    };

    Ok(matches)
}

/// Search filesystem paths and stream each matching font through `tx` as found.
///
/// Results are not sorted -- they arrive in processing order. Use this for
/// line-oriented output formats (plain text, paths, NDJSON) where the user
/// benefits from seeing results immediately.
///
/// Files that can't be read or parsed are skipped with a warning to stderr.
pub fn search_streaming(
    paths: &[PathBuf],
    query: &Query,
    opts: &SearchOptions,
    tx: Sender<TypgFontFaceMatch>,
) -> Result<()> {
    let discovery = PathDiscovery::new(paths.iter().cloned()).follow_symlinks(opts.follow_symlinks);
    let candidates = discovery.discover()?;

    let run_search = || {
        candidates
            .par_iter()
            .for_each_with(tx, |tx, loc| match load_metadata(&loc.path) {
                Ok(faces) => {
                    for face in faces {
                        if query.matches(&face.metadata) {
                            let _ = tx.send(face);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("warning: {e}");
                }
            });
    };

    if let Some(jobs) = opts.jobs {
        let pool = ThreadPoolBuilder::new().num_threads(jobs).build()?;
        pool.install(run_search);
    } else {
        run_search();
    }

    Ok(())
}

/// Filter pre-loaded entries against a query without file I/O.
pub fn filter_cached(entries: &[TypgFontFaceMatch], query: &Query) -> Vec<TypgFontFaceMatch> {
    let mut matches: Vec<TypgFontFaceMatch> = entries
        .iter()
        .filter(|entry| query.matches(&entry.metadata))
        .cloned()
        .collect();

    sort_matches(&mut matches);
    matches
}

/// Read a font file and extract metadata for each face it contains.
fn load_metadata(path: &Path) -> Result<Vec<TypgFontFaceMatch>> {
    let data = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
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
        let mut creator_names = collect_creator_names(&font);
        let mut license_names = collect_license_names(&font);

        dedup_tags(&mut axis_tags);
        dedup_tags(&mut feature_tags);
        dedup_tags(&mut script_tags);
        dedup_tags(&mut table_tags);
        dedup_codepoints(&mut codepoints);
        creator_names.sort_unstable();
        creator_names.dedup();
        license_names.sort_unstable();
        license_names.dedup();

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
                creator_names,
                license_names,
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

fn collect_creator_names(font: &FontRef) -> Vec<String> {
    let mut names = Vec::new();

    if let Ok(name_table) = font.name() {
        let data = name_table.string_data();
        let wanted = [
            NameId::COPYRIGHT_NOTICE,
            NameId::TRADEMARK,
            NameId::MANUFACTURER,
            NameId::DESIGNER,
            NameId::DESCRIPTION,
            NameId::VENDOR_URL,
            NameId::DESIGNER_URL,
            NameId::LICENSE_DESCRIPTION,
            NameId::LICENSE_URL,
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

fn collect_license_names(font: &FontRef) -> Vec<String> {
    let mut names = Vec::new();

    if let Ok(name_table) = font.name() {
        let data = name_table.string_data();
        let wanted = [
            NameId::COPYRIGHT_NOTICE,
            NameId::LICENSE_DESCRIPTION,
            NameId::LICENSE_URL,
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
