//! Font metadata extraction and search.
//!
//! This module opens font files, extracts searchable metadata from their
//! OpenType tables, and evaluates that metadata against a [`Query`]. The same
//! metadata model is used by live scans, cache files, and indexed search.
//!
//! One file may yield multiple results because collection formats such as TTC
//! and OTC can store several faces in a single container.
//!
//! Made by FontLab <https://www.fontlab.com/>
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

/// Everything we know about a single font face, extracted from its binary tables.
///
/// One font *file* may contain multiple faces (in a TTC/OTC collection), and
/// each face gets its own `TypgFontFaceMeta`. This struct is the unit of
/// comparison — every query filter is evaluated against one of these.
///
/// All tag vectors are sorted and deduplicated after extraction, so you can
/// safely use set-intersection logic against them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypgFontFaceMeta {
    /// Human-readable names for this font face.
    ///
    /// Collected from the font's `name` table: family name ("Helvetica"),
    /// typographic family name ("Helvetica Neue"), full name ("Helvetica Neue
    /// Bold"), PostScript name ("HelveticaNeue-Bold"), and subfamily
    /// ("Bold"). The file stem (e.g., "HelveticaNeue-Bold" from the filename)
    /// is always appended as a fallback, because some fonts have empty or
    /// broken name tables.
    ///
    /// Sorted, deduplicated, trimmed of whitespace.
    pub names: Vec<String>,

    /// Variation axis tags. Empty for static (non-variable) fonts.
    ///
    /// Common axes: `wght` (weight: 100=Thin, 400=Regular, 700=Bold, 900=Black),
    /// `wdth` (width: 75=Condensed, 100=Normal, 125=Expanded),
    /// `opsz` (optical size: adjusts stroke contrast for small/large rendering),
    /// `ital` (italic: 0=Upright, 1=Italic),
    /// `slnt` (slant: oblique angle in degrees).
    ///
    /// Read from the font's `fvar` (font variations) table.
    #[serde(
        serialize_with = "serialize_tags",
        deserialize_with = "deserialize_tags"
    )]
    pub axis_tags: Vec<Tag>,

    /// OpenType layout feature tags from GSUB and GPOS tables.
    ///
    /// These control typographic behavior: `liga` (standard ligatures — fi, fl
    /// become single glyphs), `smcp` (small capitals), `onum` (oldstyle
    /// numerals), `kern` (kerning — fine-tuned spacing between specific letter
    /// pairs), `calt` (contextual alternates), `dlig` (discretionary ligatures).
    ///
    /// GSUB features handle glyph *substitution* (replacing one glyph with
    /// another). GPOS features handle glyph *positioning* (adjusting placement).
    /// Both are merged here because the query doesn't distinguish them.
    #[serde(
        serialize_with = "serialize_tags",
        deserialize_with = "deserialize_tags"
    )]
    pub feature_tags: Vec<Tag>,

    /// Script tags declaring which writing systems this font supports.
    ///
    /// Read from GSUB and GPOS script lists. Common values: `latn` (Latin),
    /// `arab` (Arabic), `cyrl` (Cyrillic), `grek` (Greek), `hani` (CJK
    /// ideographs), `deva` (Devanagari), `thai` (Thai).
    ///
    /// A font can render characters from a script's Unicode range without
    /// declaring script support here — the script tag means the font has
    /// *shaping rules* (substitutions, positioning) specifically for that
    /// writing system.
    #[serde(
        serialize_with = "serialize_tags",
        deserialize_with = "deserialize_tags"
    )]
    pub script_tags: Vec<Tag>,

    /// Every top-level table present in the font file.
    ///
    /// Useful for structural queries: does this font have `CFF ` (PostScript
    /// outlines) or `glyf` (TrueType outlines)? Does it have `SVG ` (color
    /// SVG glyphs) or `COLR` (color layer glyphs)? Does it have `fvar`
    /// (variable font axes)?
    ///
    /// Read directly from the font's table directory — the index at the
    /// start of every OpenType file.
    #[serde(
        serialize_with = "serialize_tags",
        deserialize_with = "deserialize_tags"
    )]
    pub table_tags: Vec<Tag>,

    /// Unicode codepoints this font can render, from its `cmap` table.
    ///
    /// The `cmap` (character map) is the font's promise: "give me this
    /// Unicode codepoint, I'll give you a glyph." If U+00F1 (ñ) is in
    /// this list, the font has a glyph for it.
    ///
    /// Sorted and deduplicated. Can be large — a CJK font may cover
    /// 20,000+ codepoints.
    pub codepoints: Vec<char>,

    /// Whether this font has an `fvar` table, making it a variable font.
    ///
    /// Variable fonts contain continuous design axes (weight, width, etc.)
    /// instead of discrete named instances. A single variable font file can
    /// replace an entire family of static fonts.
    pub is_variable: bool,

    /// OS/2 `usWeightClass` value. Indicates visual weight on a 1–1000 scale.
    ///
    /// Standard values: 100=Thin, 200=ExtraLight, 300=Light, 400=Regular,
    /// 500=Medium, 600=SemiBold, 700=Bold, 800=ExtraBold, 900=Black.
    /// `None` if the font has no OS/2 table (rare in modern fonts).
    #[serde(default)]
    pub weight_class: Option<u16>,

    /// OS/2 `usWidthClass` value. Indicates visual width on a 1–9 scale.
    ///
    /// Values: 1=UltraCondensed, 2=ExtraCondensed, 3=Condensed,
    /// 4=SemiCondensed, 5=Normal, 6=SemiExpanded, 7=Expanded,
    /// 8=ExtraExpanded, 9=UltraExpanded.
    /// `None` if the font has no OS/2 table.
    #[serde(default)]
    pub width_class: Option<u16>,

    /// OS/2 `sFamilyClass` split into (major class, subclass).
    ///
    /// The major class groups fonts by general style: 0=No classification,
    /// 1=Oldstyle Serifs, 2=Transitional Serifs, 3=Modern Serifs,
    /// 4=Clarendon Serifs, 5=Slab Serifs, 7=Freeform Serifs,
    /// 8=Sans Serif, 9=Ornamentals, 10=Scripts, 12=Symbolic.
    ///
    /// The subclass provides finer detail within each major class.
    /// For example, within Sans Serif (8): 1=IBM Neo-Grotesque Gothic,
    /// 2=Humanist, 3=Low-x Round Geometric, etc.
    ///
    /// `None` if the font has no OS/2 table.
    #[serde(default)]
    pub family_class: Option<(u8, u8)>,

    /// Creator and provenance strings from the font's name table.
    ///
    /// Includes: copyright notice (name ID 0), trademark (7), manufacturer
    /// (8), designer (9), description (10), vendor URL (11), designer URL
    /// (12), license description (13), license URL (14).
    ///
    /// Useful for searching by foundry ("Adobe"), designer ("Matthew Carter"),
    /// or license type ("OFL").
    #[serde(default)]
    pub creator_names: Vec<String>,

    /// License-specific strings from the font's name table.
    ///
    /// A subset of creator info focused on licensing: copyright notice
    /// (name ID 0), license description (13), license info URL (14).
    ///
    /// Useful for compliance checks: "show me all fonts with an SIL Open
    /// Font License" or "find fonts with no license URL."
    #[serde(default)]
    pub license_names: Vec<String>,
}

/// Where a font face lives on disk.
///
/// For standalone `.ttf`/`.otf` files, the path is enough. For collection
/// files (`.ttc`/`.otc`) that bundle multiple faces, the `ttc_index`
/// identifies which face inside the collection this refers to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypgFontSource {
    /// Filesystem path to the font file.
    pub path: PathBuf,
    /// Face index within a TTC/OTC collection file.
    ///
    /// `None` for single-face files (`.ttf`, `.otf`).
    /// `Some(0)`, `Some(1)`, etc. for faces inside a collection.
    /// For example, a `.ttc` containing "Arial" and "Arial Bold" would have
    /// indices 0 and 1.
    pub ttc_index: Option<u32>,
}

impl TypgFontSource {
    /// Format as `path#index` for collection members, plain path otherwise.
    ///
    /// Examples: `/fonts/Noto.ttc#0`, `/fonts/Noto.ttc#1`, `/fonts/Arial.ttf`.
    /// This notation is a common convention across font tools.
    pub fn path_with_index(&self) -> String {
        if let Some(idx) = self.ttc_index {
            format!("{}#{idx}", self.path.display())
        } else {
            self.path.display().to_string()
        }
    }
}

/// A search result: one font face that matched the query.
///
/// Pairs the file location ([`TypgFontSource`]) with everything we extracted
/// from the font's binary tables ([`TypgFontFaceMeta`]). This is the primary
/// output type of the search engine — what you iterate over to display results,
/// build caches, or pipe into downstream tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypgFontFaceMatch {
    /// Where the font lives: file path and optional TTC/OTC face index.
    pub source: TypgFontSource,
    /// Metadata extracted from the font's internal tables.
    pub metadata: TypgFontFaceMeta,
}

/// Controls how the search engine runs: parallelism and traversal behavior.
#[derive(Debug, Default, Clone)]
pub struct SearchOptions {
    /// Follow symbolic links when walking directories.
    ///
    /// Off by default to avoid infinite loops from circular symlinks.
    /// Turn on when your font directories contain symlinks to real font
    /// folders (common on macOS and Linux).
    pub follow_symlinks: bool,

    /// Number of parallel worker threads for font parsing.
    ///
    /// `None` (the default) uses all available CPU cores via rayon's
    /// default thread pool. Set to `Some(1)` for single-threaded
    /// operation (useful for debugging or constrained environments).
    pub jobs: Option<usize>,
}

/// Search directories for fonts matching a query. The main entry point.
///
/// Walks the given directories, opens every font file found, extracts
/// metadata, filters against the query, and returns all matches sorted by
/// path (then by TTC index within each path).
///
/// This function collects all results in memory before returning — use
/// [`search_streaming`] if you want results delivered as they're found
/// (better for CLI output where users want to see progress immediately).
///
/// Corrupt or unreadable font files are silently skipped. The search
/// never fails because of a single bad file.
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
                Err(_) => Vec::new(),
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

/// Search directories and stream results as they're discovered.
///
/// Unlike [`search`], this doesn't wait until all fonts are processed. Each
/// match is sent through the channel (`tx`) the moment it's found. Results
/// arrive in arbitrary order — whichever thread finishes parsing a font
/// first sends its matches first.
///
/// Use this for line-oriented output (plain text, paths, NDJSON) where the
/// user benefits from seeing results immediately. The CLI's default output
/// mode uses streaming so results start appearing while the scan is still
/// running.
///
/// The sender (`tx`) is cloned across worker threads via rayon's
/// `for_each_with`. When all threads finish, every clone is dropped, which
/// closes the channel — the receiver knows the search is complete.
///
/// Corrupt or unreadable font files are silently skipped.
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
                Err(_) => {}
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

/// Filter pre-loaded font metadata against a query. No disk I/O.
///
/// Takes a slice of already-extracted font metadata (typically loaded from
/// a JSON cache file) and returns only the entries matching the query.
/// Results are sorted by path.
///
/// This is the fast path for repeated queries: pay the cost of scanning
/// and extracting once (via [`search`] or the CLI's `cache add` command),
/// save the results to a JSON file, then filter them in memory as many
/// times as you like.
pub fn filter_cached(entries: &[TypgFontFaceMatch], query: &Query) -> Vec<TypgFontFaceMatch> {
    let mut matches: Vec<TypgFontFaceMatch> = entries
        .iter()
        .filter(|entry| query.matches(&entry.metadata))
        .cloned()
        .collect();

    sort_matches(&mut matches);
    matches
}

/// Read a font file and extract metadata for every face it contains.
///
/// A standalone `.ttf`/`.otf` file yields one `TypgFontFaceMatch`.
/// A collection file (`.ttc`/`.otc`) yields one per face — a file with
/// 12 faces produces 12 results.
///
/// Uses `read-fonts` for low-level table access (axes, features, scripts,
/// tables, names, OS/2 classification) and `skrifa` for higher-level APIs
/// (cmap/charmap iteration). Both crates come from Google's fontations
/// project.
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

/// List every top-level table tag in the font's table directory.
///
/// The table directory is the index at the start of every OpenType file.
/// It maps four-character tags to byte offsets. Common tables: `glyf`
/// (TrueType outlines), `CFF ` (PostScript outlines), `GSUB`, `GPOS`,
/// `OS/2`, `name`, `cmap`, `head`, `fvar` (variable font axes).
fn collect_tables(font: &FontRef) -> Vec<Tag> {
    font.table_directory
        .table_records()
        .iter()
        .map(|rec| rec.tag())
        .collect()
}

/// Extract variation axis tags from the `fvar` table.
///
/// Returns an empty vec for static (non-variable) fonts. For variable fonts,
/// returns tags like `wght`, `wdth`, `opsz`, `ital`, `slnt`, plus any
/// custom axes the designer defined.
fn collect_axes(font: &FontRef) -> Vec<Tag> {
    if let Ok(fvar) = font.fvar() {
        if let Ok(axes) = fvar.axes() {
            return axes.iter().map(|axis| axis.axis_tag()).collect();
        }
    }
    Vec::new()
}

/// Collect OpenType feature tags from GSUB and GPOS tables.
///
/// GSUB (glyph substitution) holds features like `liga` (ligatures), `smcp`
/// (small caps), `calt` (contextual alternates). GPOS (glyph positioning)
/// holds features like `kern` (kerning), `mark` (mark-to-base positioning).
/// Both tables' feature lists are merged into one flat vec.
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

/// Collect script tags from GSUB and GPOS tables.
///
/// Script tags identify writing systems: `latn` (Latin), `arab` (Arabic),
/// `cyrl` (Cyrillic), `hani` (CJK ideographs), `deva` (Devanagari).
/// A font declares script support when it has shaping rules (lookups)
/// specifically written for that script's typographic conventions.
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

/// Extract all Unicode codepoints from the font's `cmap` table.
///
/// The `cmap` (character map) maps Unicode codepoints to glyph IDs. If a
/// codepoint appears here, the font has a glyph for it. We use `skrifa`'s
/// `charmap().mappings()` iterator, which walks every (codepoint, glyph_id)
/// pair in the font's best available cmap subtable.
///
/// Invalid Unicode scalar values (surrogates, out-of-range) are silently
/// skipped via `char::from_u32`.
fn collect_codepoints(font: &SkrifaFontRef) -> Vec<char> {
    let mut cps = Vec::new();
    for (cp, _) in font.charmap().mappings() {
        if let Some(ch) = char::from_u32(cp) {
            cps.push(ch);
        }
    }
    cps
}

/// Extract identifying name strings from the font's `name` table.
///
/// The `name` table stores human-readable strings in multiple languages and
/// encodings. We read only Unicode-encoded records for these name IDs:
///
/// - **Family Name** (ID 1): e.g., "Helvetica Neue"
/// - **Typographic Family Name** (ID 16): preferred family grouping
/// - **Subfamily Name** (ID 2): e.g., "Bold Italic"
/// - **Typographic Subfamily Name** (ID 17): preferred style name
/// - **Full Name** (ID 4): e.g., "Helvetica Neue Bold Italic"
/// - **PostScript Name** (ID 6): e.g., "HelveticaNeue-BoldItalic"
///
/// Non-Unicode records (legacy Mac Roman, Windows symbol) are skipped.
/// Empty or whitespace-only strings are discarded.
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

/// Extract creator and provenance strings from the `name` table.
///
/// Covers a broad range of attribution fields: copyright notice (ID 0),
/// trademark (7), manufacturer/foundry (8), designer (9), description (10),
/// vendor URL (11), designer URL (12), license description (13), license
/// info URL (14). These let users search by foundry, designer, or license.
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

/// Extract license-specific strings from the `name` table.
///
/// A focused subset of creator info: copyright (ID 0), license description
/// (13), and license URL (14). Separated from the broader creator fields
/// so callers can search specifically by license terms.
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

/// Extract weight class, width class, and family class from the OS/2 table.
///
/// The OS/2 table (named after IBM's OS/2 operating system — the name
/// outlived the OS by decades) carries font classification metadata:
///
/// - `usWeightClass`: visual weight, 100 (Thin) to 900 (Black).
/// - `usWidthClass`: visual width, 1 (UltraCondensed) to 9 (UltraExpanded).
/// - `sFamilyClass`: a 16-bit value where the high byte is the major class
///   (e.g., 8 = Sans Serif) and the low byte is the subclass (e.g., 1 =
///   IBM Neo-Grotesque Gothic). We split it into `(major, subclass)`.
///
/// Returns `(None, None, None)` if the font lacks an OS/2 table (very rare
/// in modern fonts, but possible in legacy or stripped files).
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

/// Sort results by file path, then by TTC index within each file.
/// Produces deterministic output regardless of thread scheduling order.
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

/// Deduplicate name strings and add a filename-based fallback.
///
/// The file stem (e.g., "HelveticaNeue-Bold" from "HelveticaNeue-Bold.otf")
/// is always appended. This ensures every font has at least one searchable
/// name, even if its `name` table is empty or broken.
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

/// Serialize OpenType tags as human-readable strings in JSON.
///
/// Tags are stored as binary `Tag` values internally, but serialized as
/// four-character strings (`"wght"`, `"liga"`) for readability in JSON
/// output and cache files.
fn serialize_tags<S>(tags: &[Tag], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let as_strings: Vec<String> = tags.iter().copied().map(tag_to_string).collect();
    as_strings.serialize(serializer)
}

/// Deserialize OpenType tags from their string representation back to `Tag` values.
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
