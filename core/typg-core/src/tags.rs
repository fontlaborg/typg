/// The secret language of font tags, decoded for human understanding
///
/// Font tags are like the secret passwords of typography - mysterious 4-letter
/// codes that font professionals whisper to each other. We provide the tools
/// to both encode your human intentions into these cryptic tags and decode
/// their wisdom back into words we can all understand.
///
/// Made with curiosity at FontLab https://www.fontlab.com/
use anyhow::{anyhow, Result};
use read_fonts::types::Tag;

/// Translates your human words into the secret language of font tags
///
/// Take your friendly characters (1-4 of them, like "wght" or "GSUB")
/// and encode them into the cryptic 4-byte format that fonts actually
/// speak. We're like the friendly translator at the United Nations
/// of Typography, making sure everyone understands each other.
///
/// Returns: A proper Tag that fonts will recognize and respect.
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

/// Unlocks the wisdom hidden in font tags, revealing them to human eyes
///
/// Those mysterious 4-byte codes that fonts chatter about? We translate
/// them back into the readable strings we humans can understand.
/// It's like being fluent in both Elvish and Common Tongue - you can
/// talk to the fonts and then tell everyone what they said.
///
/// Returns: The human-readable version of whatever the font was trying to say.
pub fn tag_to_string(tag: Tag) -> String {
    String::from_utf8_lossy(&tag.to_be_bytes()).to_string()
}
