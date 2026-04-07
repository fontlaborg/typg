/// Query construction and evaluation for font search.
///
/// A [`Query`] holds filter criteria (tag lists, name regexes, codepoint sets,
/// weight/width ranges, family class, variable-only flag). Call `matches()` to
/// test a [`TypgFontFaceMeta`] against the query.
///
/// Made by FontLab https://www.fontlab.com/
use std::collections::{HashMap, HashSet};
use std::ops::RangeInclusive;

use anyhow::{anyhow, Result};
use read_fonts::types::Tag;
use regex::Regex;

use crate::search::TypgFontFaceMeta;
use crate::tags::tag4;

/// Filter criteria for font search.
///
/// Built via chained `with_*` methods. An empty query matches all fonts.
#[derive(Debug, Clone, Default)]
pub struct Query {
    /// Required variation axis tags.
    axes: Vec<Tag>,
    /// Required OpenType feature tags.
    features: Vec<Tag>,
    /// Required script tags.
    scripts: Vec<Tag>,
    /// Required table tags.
    tables: Vec<Tag>,
    /// Regex patterns that must match at least one name string.
    name_patterns: Vec<Regex>,
    /// Unicode codepoints the font must cover.
    codepoints: Vec<char>,
    /// If true, only match variable fonts.
    variable_only: bool,
    /// Required OS/2 weight class range.
    weight_range: Option<RangeInclusive<u16>>,
    /// Required OS/2 width class range.
    width_range: Option<RangeInclusive<u16>>,
    /// Required OS/2 family class filter.
    family_class: Option<FamilyClassFilter>,
    /// Regex patterns that must match creator-related name strings.
    creator_patterns: Vec<Regex>,
    /// Regex patterns that must match license-related name strings.
    license_patterns: Vec<Regex>,
}

impl Query {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_axes(mut self, axes: Vec<Tag>) -> Self {
        self.axes = axes;
        self
    }

    pub fn with_features(mut self, features: Vec<Tag>) -> Self {
        self.features = features;
        self
    }

    pub fn with_scripts(mut self, scripts: Vec<Tag>) -> Self {
        self.scripts = scripts;
        self
    }

    pub fn with_tables(mut self, tables: Vec<Tag>) -> Self {
        self.tables = tables;
        self
    }

    pub fn with_name_patterns(mut self, patterns: Vec<Regex>) -> Self {
        self.name_patterns = patterns;
        self
    }

    pub fn with_codepoints(mut self, cps: Vec<char>) -> Self {
        self.codepoints = cps;
        self
    }

    pub fn require_variable(mut self, yes: bool) -> Self {
        self.variable_only = yes;
        self
    }

    pub fn with_weight_range(mut self, range: Option<RangeInclusive<u16>>) -> Self {
        self.weight_range = range;
        self
    }

    pub fn with_width_range(mut self, range: Option<RangeInclusive<u16>>) -> Self {
        self.width_range = range;
        self
    }

    pub fn with_family_class(mut self, class: Option<FamilyClassFilter>) -> Self {
        self.family_class = class;
        self
    }

    pub fn with_creator_patterns(mut self, patterns: Vec<Regex>) -> Self {
        self.creator_patterns = patterns;
        self
    }

    pub fn with_license_patterns(mut self, patterns: Vec<Regex>) -> Self {
        self.license_patterns = patterns;
        self
    }

    // Accessor methods for use by the high-performance index module.

    /// Get the required axis tags.
    pub fn axes(&self) -> &[Tag] {
        &self.axes
    }

    /// Get the required feature tags.
    pub fn features(&self) -> &[Tag] {
        &self.features
    }

    /// Get the required script tags.
    pub fn scripts(&self) -> &[Tag] {
        &self.scripts
    }

    /// Get the required table tags.
    pub fn tables(&self) -> &[Tag] {
        &self.tables
    }

    /// Get the name patterns.
    pub fn name_patterns(&self) -> &[Regex] {
        &self.name_patterns
    }

    /// Get the required codepoints.
    pub fn codepoints(&self) -> &[char] {
        &self.codepoints
    }

    /// Check if variable fonts are required.
    pub fn requires_variable(&self) -> bool {
        self.variable_only
    }

    /// Get the weight range filter.
    pub fn weight_range(&self) -> Option<&RangeInclusive<u16>> {
        self.weight_range.as_ref()
    }

    /// Get the width range filter.
    pub fn width_range(&self) -> Option<&RangeInclusive<u16>> {
        self.width_range.as_ref()
    }

    /// Get the family class filter.
    pub fn family_class(&self) -> Option<&FamilyClassFilter> {
        self.family_class.as_ref()
    }

    /// Get the creator patterns.
    pub fn creator_patterns(&self) -> &[Regex] {
        &self.creator_patterns
    }

    /// Get the license patterns.
    pub fn license_patterns(&self) -> &[Regex] {
        &self.license_patterns
    }

    /// Returns true if `meta` satisfies all criteria in this query.
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
