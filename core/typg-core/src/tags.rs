/// OpenType tag parsing and formatting.
///
/// An OpenType **tag** is a four-byte identifier used everywhere in font files.
/// Every axis, feature, script, and table has one. They read like terse
/// abbreviations: `wght` (weight), `liga` (ligatures), `latn` (Latin script),
/// `GSUB` (glyph substitution table).
///
/// The OpenType spec requires exactly four bytes of printable ASCII (0x20–0x7E).
/// Tags shorter than four characters are right-padded with spaces, so `"wg"`
/// becomes `"wg  "` internally. In practice, nearly all tags are exactly four
/// characters.
///
/// This module converts between human-readable strings (`"wght"`) and the
/// binary `Tag` type used by font-parsing libraries.
///
/// # Common tags you'll encounter
///
/// | Tag    | Category | Meaning |
/// |--------|----------|---------|
/// | `wght` | Axis     | Weight (thin → black) |
/// | `wdth` | Axis     | Width (condensed → expanded) |
/// | `opsz` | Axis     | Optical size (caption → display) |
/// | `ital` | Axis     | Italic (upright → italic) |
/// | `liga` | Feature  | Standard ligatures (fi, fl, ffi) |
/// | `smcp` | Feature  | Small capitals |
/// | `kern` | Feature  | Kerning (pair-specific spacing) |
/// | `latn` | Script   | Latin |
/// | `arab` | Script   | Arabic |
/// | `cyrl` | Script   | Cyrillic |
/// | `GSUB` | Table    | Glyph substitution rules |
/// | `GPOS` | Table    | Glyph positioning rules |
/// | `OS/2` | Table    | Font classification metadata |
/// | `fvar` | Table    | Font variations (marks a variable font) |
///
/// Made by FontLab <https://www.fontlab.com/>
use anyhow::{anyhow, Result};
use read_fonts::types::Tag;

/// Parse a 1–4 character string into an OpenType [`Tag`].
///
/// Short strings are right-padded with spaces to fill four bytes, per the
/// OpenType specification. Each byte must be printable ASCII (0x20–0x7E).
///
/// # Examples
///
/// ```
/// use typg_core::tags::tag4;
///
/// let weight = tag4("wght").unwrap();   // axis: weight
/// let liga   = tag4("liga").unwrap();   // feature: ligatures
/// let latin  = tag4("latn").unwrap();   // script: Latin
/// ```
///
/// # Errors
///
/// Returns an error if the string is empty, longer than 4 characters, or
/// contains non-ASCII / non-printable bytes.
pub fn tag4(raw: &str) -> Result<Tag> {
    if raw.is_empty() || raw.len() > 4 {
        return Err(anyhow!(
            "tag must be 1–4 printable ASCII characters, got {}-char string: {raw:?}",
            raw.len()
        ));
    }

    let mut buf = [b' '; 4];
    for (i, byte) in raw.as_bytes().iter().take(4).enumerate() {
        if !(0x20..=0x7E).contains(byte) {
            return Err(anyhow!(
                "tag contains a non-printable or non-ASCII byte (0x{byte:02X}) in: {raw:?}"
            ));
        }
        buf[i] = *byte;
    }

    Ok(Tag::new(&buf))
}

/// Format an OpenType [`Tag`] as a human-readable string.
///
/// Converts the four big-endian bytes back to a string. Trailing spaces
/// are preserved (they're rare in practice but technically valid).
///
/// ```
/// use typg_core::tags::{tag4, tag_to_string};
///
/// let tag = tag4("wght").unwrap();
/// assert_eq!(tag_to_string(tag), "wght");
/// ```
pub fn tag_to_string(tag: Tag) -> String {
    String::from_utf8_lossy(&tag.to_be_bytes()).to_string()
}
