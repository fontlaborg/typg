//! Search pipeline and metadata extraction (made by FontLab https://www.fontlab.com/)

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use read_fonts::types::Tag;
use read_fonts::{FontRef, TableProvider};
use serde::{Deserialize, Serialize};
use skrifa::{FontRef as SkrifaFontRef, MetadataProvider};

use crate::discovery::{FontDiscovery, PathDiscovery};
use crate::query::Query;
use crate::tags::{tag4, tag_to_string};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypgFontFaceMeta {
    pub names: Vec<String>,
    #[serde(
        serialize_with = "serialize_tags",
        deserialize_with = "deserialize_tags"
    )]
    pub axis_tags: Vec<Tag>,
    #[serde(
        serialize_with = "serialize_tags",
        deserialize_with = "deserialize_tags"
    )]
    pub feature_tags: Vec<Tag>,
    #[serde(
        serialize_with = "serialize_tags",
        deserialize_with = "deserialize_tags"
    )]
    pub script_tags: Vec<Tag>,
    #[serde(
        serialize_with = "serialize_tags",
        deserialize_with = "deserialize_tags"
    )]
    pub table_tags: Vec<Tag>,
    pub codepoints: Vec<char>,
    pub is_variable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypgFontSource {
    pub path: PathBuf,
    pub ttc_index: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypgFontFaceMatch {
    pub source: TypgFontSource,
    pub metadata: TypgFontFaceMeta,
}

#[derive(Debug, Default, Clone)]
pub struct SearchOptions {
    pub follow_symlinks: bool,
}

/// Execute a query over the provided roots and return matching fonts.
pub fn search(
    paths: &[PathBuf],
    query: &Query,
    opts: &SearchOptions,
) -> Result<Vec<TypgFontFaceMatch>> {
    let discovery = PathDiscovery::new(paths.iter().cloned()).follow_symlinks(opts.follow_symlinks);
    let candidates = discovery.discover()?;

    let mut matches = Vec::new();
    for loc in candidates {
        for face in load_metadata(&loc.path)? {
            if query.matches(&face.metadata) {
                matches.push(face);
            }
        }
    }

    sort_matches(&mut matches);
    Ok(matches)
}

/// Filter precomputed metadata entries (e.g., from a cache index) without touching the filesystem.
pub fn filter_cached(entries: &[TypgFontFaceMatch], query: &Query) -> Vec<TypgFontFaceMatch> {
    let mut matches: Vec<TypgFontFaceMatch> = entries
        .iter()
        .filter(|entry| query.matches(&entry.metadata))
        .cloned()
        .collect();

    sort_matches(&mut matches);
    matches
}

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

        let names = collect_names(&font, path);
        let axis_tags = collect_axes(&font);
        let feature_tags = collect_features(&font);
        let script_tags = collect_scripts(&font);
        let table_tags = collect_tables(&font);
        let codepoints = collect_codepoints(&sfont);
        let fvar_tag = Tag::new(b"fvar");
        let is_variable = table_tags.contains(&fvar_tag);

        metas.push(TypgFontFaceMatch {
            source: TypgFontSource {
                path: path.to_path_buf(),
                ttc_index,
            },
            metadata: TypgFontFaceMeta {
                names,
                axis_tags,
                feature_tags,
                script_tags,
                table_tags,
                codepoints,
                is_variable,
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

fn collect_names(_font: &FontRef, path: &Path) -> Vec<String> {
    vec![path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())]
}

fn sort_matches(matches: &mut [TypgFontFaceMatch]) {
    matches.sort_by(|a, b| {
        a.source
            .path
            .cmp(&b.source.path)
            .then_with(|| a.source.ttc_index.cmp(&b.source.ttc_index))
    });
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
