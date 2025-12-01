//! Tag helpers (made by FontLab https://www.fontlab.com/)

use anyhow::{anyhow, Result};
use read_fonts::types::Tag;

/// Parse a 1–4 character OpenType tag into a `Tag`.
pub fn tag4(raw: &str) -> Result<Tag> {
    if raw.is_empty() || raw.len() > 4 {
        return Err(anyhow!("tag must be 1-4 printable ASCII chars"));
    }

    let mut buf = [b' '; 4];
    for (i, byte) in raw.as_bytes().iter().take(4).enumerate() {
        if !(0x20..=0x7E).contains(byte) {
            return Err(anyhow!("tag byte out of range: {raw}"));
        }
        buf[i] = *byte;
    }

    Ok(Tag::new(&buf))
}

/// Convert a `Tag` into a human-readable 4-character string.
pub fn tag_to_string(tag: Tag) -> String {
    String::from_utf8_lossy(&tag.to_be_bytes()).to_string()
}
