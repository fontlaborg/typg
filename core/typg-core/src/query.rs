//! Query construction and evaluation for typg.
//!
//! A [`Query`] describes the font you want. An empty query matches every font.
//! When multiple filters are set, they combine with AND logic: a match must
//! satisfy every active criterion.
//!
//! The same query model is reused for live scans, cached searches, and indexed
//! searches, so filter behavior stays consistent across the CLI, HTTP server,
//! and Python bindings.
//!
//! Made by FontLab <https://www.fontlab.com/>
use std::collections::{HashMap, HashSet};
use std::ops::RangeInclusive;

use anyhow::{anyhow, Result};
use read_fonts::types::Tag;
use regex::Regex;

use crate::search::TypgFontFaceMeta;
use crate::tags::tag4;

/// Filter criteria for font search. Built with chained `with_*` methods.
///
/// Every field is optional and defaults to "no constraint." An empty `Query`
/// matches all fonts. As you add criteria, the filter becomes more selective —
/// all criteria must be satisfied (AND logic).
///
/// The query doesn't touch the filesystem; it only evaluates in-memory
/// [`TypgFontFaceMeta`] structs. This makes it reusable across live search,
/// cached search, and indexed search without modification.
#[derive(Debug, Clone, Default)]
pub struct Query {
    /// Variation axis tags the font must define (e.g., `wght`, `wdth`).
    /// A font matches only if it has *all* listed axes.
    axes: Vec<Tag>,

    /// OpenType feature tags the font must support (e.g., `liga`, `smcp`).
    /// Checked against features from both GSUB and GPOS tables.
    features: Vec<Tag>,

    /// Script tags the font must declare support for (e.g., `latn`, `arab`).
    /// Checked against script lists in GSUB and GPOS.
    scripts: Vec<Tag>,

    /// Top-level table tags the font must contain (e.g., `GSUB`, `CFF `).
    tables: Vec<Tag>,

    /// Regex patterns tested against the font's name strings.
    /// At least one name must match at least one pattern.
    /// Use for searches like "find fonts whose name contains 'Mono'."
    name_patterns: Vec<Regex>,

    /// Unicode codepoints the font must cover (via its `cmap` table).
    /// The font must have glyphs for *all* listed codepoints.
    codepoints: Vec<char>,

    /// When `true`, only variable fonts (those with an `fvar` table) match.
    /// When `false` (default), both static and variable fonts can match.
    variable_only: bool,

    /// OS/2 `usWeightClass` must fall within this range.
    /// Standard range: 100 (Thin) to 900 (Black). `None` = no constraint.
    weight_range: Option<RangeInclusive<u16>>,

    /// OS/2 `usWidthClass` must fall within this range.
    /// Standard range: 1 (UltraCondensed) to 9 (UltraExpanded). `None` = no constraint.
    width_range: Option<RangeInclusive<u16>>,

    /// OS/2 family class (major, optionally subclass) the font must match.
    /// Example: major=8 matches all sans-serif fonts; major=8, subclass=1
    /// matches only IBM Neo-Grotesque Gothic. `None` = no constraint.
    family_class: Option<FamilyClassFilter>,

    /// Regex patterns tested against creator/provenance name strings
    /// (copyright, trademark, manufacturer, designer, description, URLs,
    /// license text). At least one creator string must match at least one
    /// pattern.
    creator_patterns: Vec<Regex>,

    /// Regex patterns tested against license-specific name strings
    /// (copyright, license description, license URL). At least one license
    /// string must match at least one pattern.
    license_patterns: Vec<Regex>,
}

impl Query {
    /// Create an empty query that matches all fonts.
    pub fn new() -> Self {
        Self::default()
    }

    /// Require these variation axes. A font must define *all* of them.
    /// Example: `vec![tag4("wght")?, tag4("wdth")?]`
    pub fn with_axes(mut self, axes: Vec<Tag>) -> Self {
        self.axes = axes;
        self
    }

    /// Require these OpenType features. The font must list *all* of them.
    /// Example: `vec![tag4("liga")?, tag4("smcp")?]`
    pub fn with_features(mut self, features: Vec<Tag>) -> Self {
        self.features = features;
        self
    }

    /// Require these script tags. The font must declare support for *all*.
    /// Example: `vec![tag4("latn")?, tag4("cyrl")?]`
    pub fn with_scripts(mut self, scripts: Vec<Tag>) -> Self {
        self.scripts = scripts;
        self
    }

    /// Require these top-level tables. The font file must contain *all*.
    /// Example: `vec![tag4("GSUB")?, tag4("GPOS")?]`
    pub fn with_tables(mut self, tables: Vec<Tag>) -> Self {
        self.tables = tables;
        self
    }

    /// Require at least one font name to match at least one regex pattern.
    /// Patterns are tested against all name strings (family, full, PostScript, etc.).
    pub fn with_name_patterns(mut self, patterns: Vec<Regex>) -> Self {
        self.name_patterns = patterns;
        self
    }

    /// Require the font to have glyphs for *all* of these Unicode codepoints.
    /// Example: `vec!['A', 'B', 'ñ']`
    pub fn with_codepoints(mut self, cps: Vec<char>) -> Self {
        self.codepoints = cps;
        self
    }

    /// When `true`, only variable fonts match. Default: `false` (both match).
    pub fn require_variable(mut self, yes: bool) -> Self {
        self.variable_only = yes;
        self
    }

    /// Require OS/2 weight class within this range. Example: `Some(300..=700)`.
    pub fn with_weight_range(mut self, range: Option<RangeInclusive<u16>>) -> Self {
        self.weight_range = range;
        self
    }

    /// Require OS/2 width class within this range. Example: `Some(3..=7)`.
    pub fn with_width_range(mut self, range: Option<RangeInclusive<u16>>) -> Self {
        self.width_range = range;
        self
    }

    /// Require a specific OS/2 family class (and optionally subclass).
    pub fn with_family_class(mut self, class: Option<FamilyClassFilter>) -> Self {
        self.family_class = class;
        self
    }

    /// Require at least one creator string to match at least one regex.
    pub fn with_creator_patterns(mut self, patterns: Vec<Regex>) -> Self {
        self.creator_patterns = patterns;
        self
    }

    /// Require at least one license string to match at least one regex.
    pub fn with_license_patterns(mut self, patterns: Vec<Regex>) -> Self {
        self.license_patterns = patterns;
        self
    }

    /// The required axis tags, if any.
    pub fn axes(&self) -> &[Tag] {
        &self.axes
    }

    /// The required feature tags, if any.
    pub fn features(&self) -> &[Tag] {
        &self.features
    }

    /// The required script tags, if any.
    pub fn scripts(&self) -> &[Tag] {
        &self.scripts
    }

    /// The required table tags, if any.
    pub fn tables(&self) -> &[Tag] {
        &self.tables
    }

    /// The name regex patterns, if any.
    pub fn name_patterns(&self) -> &[Regex] {
        &self.name_patterns
    }

    /// The required codepoints, if any.
    pub fn codepoints(&self) -> &[char] {
        &self.codepoints
    }

    /// Whether only variable fonts are accepted.
    pub fn requires_variable(&self) -> bool {
        self.variable_only
    }

    /// The weight class range constraint, if set.
    pub fn weight_range(&self) -> Option<&RangeInclusive<u16>> {
        self.weight_range.as_ref()
    }

    /// The width class range constraint, if set.
    pub fn width_range(&self) -> Option<&RangeInclusive<u16>> {
        self.width_range.as_ref()
    }

    /// The family class constraint, if set.
    pub fn family_class(&self) -> Option<&FamilyClassFilter> {
        self.family_class.as_ref()
    }

    /// The creator/provenance regex patterns, if any.
    pub fn creator_patterns(&self) -> &[Regex] {
        &self.creator_patterns
    }

    /// The license regex patterns, if any.
    pub fn license_patterns(&self) -> &[Regex] {
        &self.license_patterns
    }

    /// Test a font's metadata against every criterion in this query.
    ///
    /// Returns `true` only if *all* active criteria are satisfied.
    /// Criteria that aren't set (empty vecs, `None` ranges) are skipped.
    ///
    /// Evaluation order is roughly cheapest-first: boolean checks, then
    /// tag set intersections, then numeric ranges, then codepoint coverage,
    /// then regex matching (most expensive). Short-circuits on the first
    /// failure.
    pub fn matches(&self, meta: &TypgFontFaceMeta) -> bool {
        if self.variable_only && !meta.is_variable {
            return false;
        }

        if !contains_all_tags(&meta.axis_tags, &self.axes) {
            return false;
        }

        if !contains_all_tags(&meta.feature_tags, &self.features) {
            return false;
        }

        if !contains_all_tags(&meta.script_tags, &self.scripts) {
            return false;
        }

        if !contains_all_tags(&meta.table_tags, &self.tables) {
            return false;
        }

        if let Some(range) = &self.weight_range {
            match meta.weight_class {
                Some(weight) if range.contains(&weight) => {}
                _ => return false,
            }
        }

        if let Some(range) = &self.width_range {
            match meta.width_class {
                Some(width) if range.contains(&width) => {}
                _ => return false,
            }
        }

        if let Some(filter) = &self.family_class {
            match meta.family_class {
                Some((class, subclass)) => {
                    if class != filter.major {
                        return false;
                    }
                    if let Some(expected_subclass) = filter.subclass {
                        if subclass != expected_subclass {
                            return false;
                        }
                    }
                }
                None => return false,
            }
        }

        if !self.codepoints.is_empty() {
            let available: HashSet<char> = meta.codepoints.iter().copied().collect();
            if !self.codepoints.iter().all(|cp| available.contains(cp)) {
                return false;
            }
        }

        if !self.name_patterns.is_empty() {
            let matched = meta
                .names
                .iter()
                .any(|name| self.name_patterns.iter().any(|re| re.is_match(name)));
            if !matched {
                return false;
            }
        }

        if !self.creator_patterns.is_empty() {
            let matched = meta
                .creator_names
                .iter()
                .any(|name| self.creator_patterns.iter().any(|re| re.is_match(name)));
            if !matched {
                return false;
            }
        }

        if !self.license_patterns.is_empty() {
            let matched = meta
                .license_names
                .iter()
                .any(|name| self.license_patterns.iter().any(|re| re.is_match(name)));
            if !matched {
                return false;
            }
        }

        true
    }
}

/// Check that `haystack` contains every tag in `needles` (set subset check).
/// Returns `true` if `needles` is empty (vacuous truth — no requirements).
fn contains_all_tags(haystack: &[Tag], needles: &[Tag]) -> bool {
    if needles.is_empty() {
        return true;
    }
    let set: HashSet<Tag> = haystack.iter().copied().collect();
    needles.iter().all(|tag| set.contains(tag))
}

/// Parse a comma-separated list of codepoints or ranges into a `Vec<char>`.
///
/// Accepts single characters ("A"), Unicode escapes ("U+0041"), and ranges
/// ("A-Z", "U+0041-U+005A"), or any comma-separated combination thereof.
pub fn parse_codepoint_list(input: &str) -> Result<Vec<char>> {
    let mut result = Vec::new();
    if input.trim().is_empty() {
        return Ok(result);
    }

    for part in input.split(',') {
        if part.contains('-') {
            let pieces: Vec<&str> = part.split('-').collect();
            if pieces.len() != 2 {
                return Err(anyhow!("invalid range: {part}"));
            }
            let start = parse_codepoint(pieces[0])? as u32;
            let end = parse_codepoint(pieces[1])? as u32;
            let (lo, hi) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            for cp in lo..=hi {
                if let Some(ch) = char::from_u32(cp) {
                    result.push(ch);
                }
            }
        } else {
            result.push(parse_codepoint(part)?);
        }
    }

    Ok(result)
}

fn parse_codepoint(token: &str) -> Result<char> {
    if token.chars().count() == 1 {
        return Ok(token.chars().next().unwrap());
    }

    let trimmed = token.trim_start_matches("U+").trim_start_matches("u+");
    let cp = u32::from_str_radix(trimmed, 16).map_err(|_| anyhow!("invalid codepoint: {token}"))?;
    char::from_u32(cp).ok_or_else(|| anyhow!("invalid Unicode scalar: U+{cp:04X}"))
}

/// Parse a slice of tag strings (e.g. `"wght"`, `"smcp"`) into `Tag` values.
///
/// Each string must be 1–4 printable ASCII characters.
pub fn parse_tag_list(raw: &[String]) -> Result<Vec<Tag>> {
    raw.iter().map(|s| tag4(s)).collect()
}

/// Filter for the OS/2 family-class field.
///
/// `major` selects a broad class such as serif, sans-serif, or script.
/// `subclass`, when present, narrows the match to one subclass inside that
/// major class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FamilyClassFilter {
    pub major: u8,
    pub subclass: Option<u8>,
}

/// Parse an OS/2 family class specifier into a [`FamilyClassFilter`].
///
/// Accepts numeric values ("8"), hex values ("0x0800"), major.subclass pairs
/// ("8.11"), and named aliases ("sans", "serif", "script", etc.).
pub fn parse_family_class(input: &str) -> Result<FamilyClassFilter> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("family class cannot be empty"));
    }

    let lower = trimmed.to_ascii_lowercase();
    if let Some(major) = lookup_family_class_by_name(&lower) {
        return Ok(FamilyClassFilter {
            major,
            subclass: None,
        });
    }

    if let Some((major, subclass)) = parse_major_and_subclass(&lower) {
        return Ok(FamilyClassFilter {
            major,
            subclass: Some(subclass),
        });
    }

    let value = if let Some(stripped) = lower.strip_prefix("0x") {
        u16::from_str_radix(stripped, 16)
            .map_err(|_| anyhow!("invalid hex family class: {trimmed}"))?
    } else {
        lower
            .parse::<u16>()
            .map_err(|_| anyhow!("invalid family class: {trimmed}"))?
    };

    if value <= 0x00FF {
        return Ok(FamilyClassFilter {
            major: value as u8,
            subclass: None,
        });
    }

    let major = (value >> 8) as u8;
    let subclass = (value & 0x00FF) as u8;

    Ok(FamilyClassFilter {
        major,
        subclass: Some(subclass),
    })
}

fn lookup_family_class_by_name(name: &str) -> Option<u8> {
    let mut map: HashMap<&str, u8> = HashMap::new();
    map.insert("none", 0);
    map.insert("no-class", 0);
    map.insert("uncategorized", 0);
    map.insert("oldstyle", 1);
    map.insert("old-style", 1);
    map.insert("oldstyle-serif", 1);
    map.insert("transitional", 2);
    map.insert("modern", 3);
    map.insert("clarendon", 4);
    map.insert("slab", 5);
    map.insert("slab-serif", 5);
    map.insert("egyptian", 5);
    map.insert("freeform", 7);
    map.insert("freeform-serif", 7);
    map.insert("sans", 8);
    map.insert("sans-serif", 8);
    map.insert("gothic", 8);
    map.insert("ornamental", 9);
    map.insert("decorative", 9);
    map.insert("script", 10);
    map.insert("symbolic", 12);
    map.get(name).copied()
}

fn parse_major_and_subclass(raw: &str) -> Option<(u8, u8)> {
    for sep in ['.', ':'] {
        if let Some((major, sub)) = raw.split_once(sep) {
            let major: u8 = major.parse().ok()?;
            let subclass: u8 = sub.parse().ok()?;
            return Some((major, subclass));
        }
    }
    None
}

/// Parse a single value or range of u16 numbers (e.g., "400" or "300-500").
pub fn parse_u16_range(input: &str) -> Result<RangeInclusive<u16>> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("range cannot be empty"));
    }

    if let Some((lo, hi)) = trimmed.split_once('-') {
        let start: u16 = lo.trim().parse()?;
        let end: u16 = hi.trim().parse()?;
        let (min, max) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        Ok(min..=max)
    } else {
        let value: u16 = trimmed.parse()?;
        Ok(value..=value)
    }
}
