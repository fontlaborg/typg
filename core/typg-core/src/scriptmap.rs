//! Script-tag resolution for the multi-wise `--scripts` filter.
//!
//! A user-supplied script tag may be an **ISO 15924** code (`Latn`, `Deva`,
//! `Latf`) or an **OpenType** script tag (`latn`, `dev2`, `arab`). Each input
//! is resolved into a [`ScriptRequirement`] carrying two independent ways a
//! font can satisfy it:
//!
//! 1. **OpenType** — a *group* of OpenType script tags. The font's GSUB/GPOS
//!    script list must contain *at least one* tag from the group. So `deva`
//!    expands to `["deva", "dev2", "dev3"]` and any one of them suffices.
//!
//! 2. **Unicode** — a single Unicode script (UAX #24). The font's `cmap` is
//!    considered to cover that script when it maps at least
//!    [`MIN_UNICODE_COVERAGE`] codepoints belonging to it.
//!
//! The whole-font decision (see [`crate::query::Query::matches`]) is: a font
//! supports the requested list if it satisfies *every* requirement through the
//! OpenType path, **or** it satisfies *every* requirement through the Unicode
//! path.
//!
//! Resolution is best-effort: when an input cannot be mapped, it is treated as
//! a literal OpenType script tag (and, where possible, as a Unicode script via
//! its title-cased spelling).
//!
//! Made by FontLab <https://www.fontlab.com/>
use std::collections::HashSet;

use read_fonts::types::Tag;
use unicode_script::{Script, UnicodeScript};

use crate::tags::tag4;

/// Minimum number of cmap codepoints in a Unicode script for the font's `cmap`
/// to count as covering that script.
pub const MIN_UNICODE_COVERAGE: usize = 4;

/// One resolved script requirement: an OpenType tag group plus an optional
/// Unicode script. Built by [`resolve_scripts`].
#[derive(Debug, Clone)]
pub struct ScriptRequirement {
    /// The original input string, preserved for diagnostics.
    input: String,
    /// OpenType script tags; the font satisfies the OpenType path if its
    /// GSUB/GPOS script list contains *any* of these.
    ot_tags: Vec<Tag>,
    /// Unicode script the font's `cmap` must cover (≥ [`MIN_UNICODE_COVERAGE`]
    /// codepoints) to satisfy the Unicode path. `None` when the input maps to
    /// no known Unicode script.
    unicode_script: Option<Script>,
}

impl ScriptRequirement {
    /// The original input tag string.
    pub fn input(&self) -> &str {
        &self.input
    }

    /// The OpenType tag group; any one present in the font satisfies the OT path.
    pub fn ot_tags(&self) -> &[Tag] {
        &self.ot_tags
    }

    /// The Unicode script the font's `cmap` must cover, if any.
    pub fn unicode_script(&self) -> Option<Script> {
        self.unicode_script
    }

    /// Whether the font's GSUB/GPOS script tags satisfy the OpenType path.
    pub fn ot_satisfied(&self, font_scripts: &HashSet<Tag>) -> bool {
        self.ot_tags.iter().any(|tag| font_scripts.contains(tag))
    }

    /// Whether the given codepoints cover this requirement's Unicode script.
    ///
    /// Returns `false` when the input mapped to no Unicode script. Otherwise,
    /// counts codepoints belonging to that script and stops early once
    /// [`MIN_UNICODE_COVERAGE`] is reached.
    pub fn unicode_satisfied<I>(&self, codepoints: I) -> bool
    where
        I: IntoIterator<Item = char>,
    {
        let Some(script) = self.unicode_script else {
            return false;
        };
        let mut count = 0usize;
        for ch in codepoints {
            if ch.script() == script {
                count += 1;
                if count >= MIN_UNICODE_COVERAGE {
                    return true;
                }
            }
        }
        false
    }
}

/// Resolve a list of raw script-tag strings into [`ScriptRequirement`]s.
///
/// Empty/whitespace-only entries are skipped. Each remaining entry maps to one
/// requirement via [`resolve_one`].
pub fn resolve_scripts(raw: &[String]) -> Vec<ScriptRequirement> {
    raw.iter()
        .filter_map(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(resolve_one(trimmed))
            }
        })
        .collect()
}

/// Resolve a single script-tag string into a [`ScriptRequirement`].
fn resolve_one(input: &str) -> ScriptRequirement {
    let key = input.to_ascii_lowercase();

    let (ot_strings, unicode_short): (Vec<&str>, Option<&str>) = match lookup(&key) {
        Some((tags, uni)) => (tags.to_vec(), Some(uni)),
        None => (vec![key.as_str()], None),
    };

    let ot_tags: Vec<Tag> = ot_strings.iter().filter_map(|t| tag4(t).ok()).collect();

    // Unicode script: explicit mapping if present, else the title-cased input
    // (works for the many scripts whose OpenType/ISO base equals the Unicode
    // 4-letter code, e.g. "latn" → "Latn", "cyrl" → "Cyrl", "hani" → "Hani").
    let unicode_script = unicode_short
        .and_then(Script::from_short_name)
        .or_else(|| Script::from_short_name(&title_case(&key)))
        // `Zzzz` and other uncoded inputs resolve to `Unknown`, which would
        // spuriously match unassigned/private-use codepoints. Treat as "none".
        .filter(|s| *s != Script::Unknown);

    ScriptRequirement {
        input: input.to_string(),
        ot_tags,
        unicode_script,
    }
}

/// Title-case a 4-letter script code: first byte upper, rest lower.
fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => {
            first.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase()
        }
        None => String::new(),
    }
}

/// Static map from a lowercased input tag to (OpenType tag group, Unicode
/// short name). Covers two cases the title-case fallback cannot:
///
/// - **Indic v1/v2/v3 shaping tags** — Devanagari, Bengali, etc. each have
///   multiple OpenType tags; any one satisfies the script.
/// - **ISO 15924 aliases** whose OpenType tag (and/or Unicode script) differs
///   from a simple lowercasing, e.g. `latf`/`latg` → `latn`.
///
/// Every alternate spelling of a script maps to the *same* full OpenType group,
/// so `dev2` and `deva` behave identically.
fn lookup(key: &str) -> Option<(&'static [&'static str], &'static str)> {
    // Indic scripts: (group, unicode short name). Extra/non-existent OpenType
    // tags in a group are harmless — no font declares them.
    const DEVA: &[&str] = &["deva", "dev2", "dev3"];
    const BENG: &[&str] = &["beng", "bng2"];
    const GUJR: &[&str] = &["gujr", "gjr2"];
    const GURU: &[&str] = &["guru", "gur2"];
    const KNDA: &[&str] = &["knda", "knd2"];
    const MLYM: &[&str] = &["mlym", "mlm2"];
    const ORYA: &[&str] = &["orya", "ory2"];
    const TAML: &[&str] = &["taml", "tml2"];
    const TELU: &[&str] = &["telu", "tel2"];

    let entry: (&[&str], &str) = match key {
        "deva" | "dev2" | "dev3" => (DEVA, "Deva"),
        "beng" | "bng2" => (BENG, "Beng"),
        "gujr" | "gjr2" => (GUJR, "Gujr"),
        "guru" | "gur2" => (GURU, "Guru"),
        "knda" | "knd2" => (KNDA, "Knda"),
        "mlym" | "mlm2" => (MLYM, "Mlym"),
        "orya" | "ory2" => (ORYA, "Orya"),
        "taml" | "tml2" => (TAML, "Taml"),
        "telu" | "tel2" => (TELU, "Telu"),
        // ISO 15924 aliases whose OpenType/Unicode mapping is not a lowercasing.
        "latf" | "latg" => (&["latn"], "Latn"),
        "aran" => (&["arab"], "Arab"),
        "syre" | "syrj" | "syrn" => (&["syrc"], "Syrc"),
        "hans" | "hant" => (&["hani"], "Hani"),
        "lao" | "laoo" => (&["lao "], "Laoo"),
        "yiii" => (&["yi  "], "Yiii"),
        "nkoo" => (&["nko "], "Nkoo"),
        _ => return None,
    };
    Some(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(req: &ScriptRequirement) -> Vec<String> {
        req.ot_tags()
            .iter()
            .map(|t| String::from_utf8_lossy(&t.to_be_bytes()).trim().to_string())
            .collect()
    }

    #[test]
    fn resolve_iso_devanagari_expands_to_ot_group() {
        let reqs = resolve_scripts(&["deva".to_string()]);
        assert_eq!(reqs.len(), 1);
        assert_eq!(tags(&reqs[0]), vec!["deva", "dev2", "dev3"]);
        assert_eq!(reqs[0].unicode_script(), Some(Script::Devanagari));
    }

    #[test]
    fn resolve_v2_tag_maps_to_same_group_and_script() {
        let reqs = resolve_scripts(&["dev2".to_string()]);
        assert_eq!(tags(&reqs[0]), vec!["deva", "dev2", "dev3"]);
        assert_eq!(reqs[0].unicode_script(), Some(Script::Devanagari));
    }

    #[test]
    fn resolve_latin_fraktur_alias_maps_to_latn() {
        let reqs = resolve_scripts(&["latf".to_string()]);
        assert_eq!(tags(&reqs[0]), vec!["latn"]);
        assert_eq!(reqs[0].unicode_script(), Some(Script::Latin));
    }

    #[test]
    fn resolve_plain_opentype_tag_via_fallback() {
        let reqs = resolve_scripts(&["latn".to_string()]);
        assert_eq!(tags(&reqs[0]), vec!["latn"]);
        assert_eq!(reqs[0].unicode_script(), Some(Script::Latin));
    }

    #[test]
    fn resolve_is_case_insensitive() {
        let reqs = resolve_scripts(&["LATN".to_string(), "Deva".to_string()]);
        assert_eq!(tags(&reqs[0]), vec!["latn"]);
        assert_eq!(reqs[0].unicode_script(), Some(Script::Latin));
        assert_eq!(reqs[1].unicode_script(), Some(Script::Devanagari));
    }

    #[test]
    fn resolve_unknown_tag_falls_back_to_literal_ot() {
        let reqs = resolve_scripts(&["zzzz".to_string()]);
        assert_eq!(tags(&reqs[0]), vec!["zzzz"]);
        assert_eq!(reqs[0].unicode_script(), None);
    }

    #[test]
    fn empty_and_blank_entries_are_skipped() {
        let reqs = resolve_scripts(&["".to_string(), "   ".to_string(), "latn".to_string()]);
        assert_eq!(reqs.len(), 1);
    }

    #[test]
    fn ot_satisfied_matches_any_group_member() {
        let reqs = resolve_scripts(&["deva".to_string()]);
        let mut font: HashSet<Tag> = HashSet::new();
        font.insert(tag4("dev2").unwrap());
        assert!(reqs[0].ot_satisfied(&font));

        let empty: HashSet<Tag> = HashSet::new();
        assert!(!reqs[0].ot_satisfied(&empty));
    }

    #[test]
    fn unicode_satisfied_needs_min_coverage() {
        let reqs = resolve_scripts(&["latn".to_string()]);
        // Three Latin letters: below threshold.
        assert!(!reqs[0].unicode_satisfied(['a', 'b', 'c']));
        // Four Latin letters: meets threshold.
        assert!(reqs[0].unicode_satisfied(['a', 'b', 'c', 'd']));
        // Non-Latin codepoints never count toward Latin.
        assert!(!reqs[0].unicode_satisfied(['α', 'β', 'γ', 'δ']));
    }
}
