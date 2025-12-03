//! Query parsing and matching (made by FontLab https://www.fontlab.com/)

use std::collections::HashSet;

use anyhow::{anyhow, Result};
use read_fonts::types::Tag;
use regex::Regex;

use crate::search::TypgFontFaceMeta;
use crate::tags::tag4;

#[derive(Debug, Clone, Default)]
pub struct Query {
    axes: Vec<Tag>,
    features: Vec<Tag>,
    scripts: Vec<Tag>,
    tables: Vec<Tag>,
    name_patterns: Vec<Regex>,
    codepoints: Vec<char>,
    variable_only: bool,
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

    /// Check whether the provided font metadata satisfies the query filters.
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

/// Parse comma-delimited codepoints and ranges (e.g. `U+0041-U+0044,B`).
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

/// Parse a collection of tag strings into `Tag`s, rejecting invalid lengths.
pub fn parse_tag_list(raw: &[String]) -> Result<Vec<Tag>> {
    raw.iter().map(|s| tag4(s)).collect()
}
