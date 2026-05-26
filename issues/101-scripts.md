```
-s, --scripts <SCRIPTS>            Require fonts to cover these script tags
```

This functionality must work in a multi-wise manner: 

1. The font supports the input list of scripts 

(a) if the corresponding OpenType script tags are found in the font’s GSUB or GPOS script list for every input script 
(b) OR if the font’s cmap covers each of the corresponding Unicode scripts for every input script.

2. The input script tags should be either ISO 15924 or OpenType script tags

3. OpenType

Convert each input script tag to all supporting OpenType script tags. So if I query for "latf,deva", then we’re converting "latf" to ["latn"] and "deva" to ["deva", "dev2", "dev3"]. And then the font’s GSUB or GPOS script list MUST support at least ONE of each group, so the font’s GSUB or GPOS script lists must support "latn" AND "deva", or "latn" AND "dev2", or "latn" AND "dev3". 

If the input script tag cannot be converted, then we treat it as an OpenType script explicitly, and check for the presence. 

4. Unicode 

We convert each input script tag to a corresponding Unicode script identifier (multiple ISO script tags may map to the same Unicode script identifier).

In our view, the cmap supports a Unicode script if the font’s cmap (SMP or BMP) contains at least 4 codepoints for that script.

Of course if the query is "latf,deva", then the font must support the Unicode script "Latin" and "Devanagari".



====

Now I have everything needed. Let me write the comprehensive answer.

There are two distinct problems here. I'll cover each with the best crates and approaches, including the key caveat about the non-1:1 relationship between ISO 15924 and OpenType tags.

***

## The Core Caveat: ISO 15924 ≠ OpenType 1:1

Before diving in: the OpenType spec explicitly states that OT script tags are **not guaranteed to match** ISO 15924 codes. OpenType predates ISO 15924, and some scripts have multiple OT tags for different shaping engine generations (e.g., `deva`/`dev2` for Devanagari, `beng`/`bng2` for Bengali). For most scripts, though, the 4-letter codes are identical (case-insensitive), so `from_iso15924_tag` in HarfBuzz/rustybuzz handles the mapping transparently.[1]

***

## ISO 15924 → OpenType Script Tag

### Option 1: `rustybuzz` (recommended)

`rustybuzz` is a complete pure-Rust port of HarfBuzz, and its `Script` type directly mirrors HarfBuzz's `hb_script_t`. The key method is `Script::from_iso15924_tag(tag: Tag) -> Option<Script>`, which performs the same conversion as HarfBuzz's `hb_script_from_iso15924_tag()`. The internal `Script` value is itself an OpenType/HarfBuzz tag, so once converted you can call `.tag()` to get the OT 4-byte tag.[2][3]

```toml
# Cargo.toml
[dependencies]
rustybuzz = "0.20"
```

```rust
use rustybuzz::{Script, Tag};

fn iso15924_to_ot_script(iso_tag: &str) -> Option<([u8; 4], Script)> {
    // Build a Tag from the ISO 15924 four-letter code (case-sensitive, title-case)
    let bytes = iso_tag.as_bytes();
    if bytes.len() != 4 {
        return None;
    }
    let tag = Tag::from_bytes(&[bytes[0], bytes[1], bytes[2], bytes[3]]);
    let script = Script::from_iso15924_tag(tag)?;
    // script.tag() returns the OpenType tag (same 4-byte structure)
    let ot = script.tag();
    let mut out = [0u8; 4];
    out.copy_from_slice(&ot.to_bytes());
    Some((out, script))
}

fn main() {
    // ISO 15924 "Deva" → OT tag "deva"
    if let Some((ot_bytes, script)) = iso15924_to_ot_script("Deva") {
        println!("OT tag: {}", std::str::from_utf8(&ot_bytes).unwrap());
        println!("Script: {:?}", script);
    }

    // Bengali: ISO "Beng" → one OT tag "beng"
    // Note: "bng2" is a separate legacy shaping tag and won't appear here
    if let Some((ot, _)) = iso15924_to_ot_script("Beng") {
        println!("Beng → {}", std::str::from_utf8(&ot).unwrap());
    }
}
```

The `Tag` type comes from `ttf-parser` (re-exported by `rustybuzz`) and has `from_bytes(&[u8; 4])` and `to_bytes()`. The `Script::from_iso15924_tag` call mirrors HarfBuzz's own lookup logic; it returns `None` for unknown or unregistered ISO codes.[3][4]

### Option 2: `unicode-script` — ISO 15924 tag as u32

If you don't need full HarfBuzz infrastructure, the lighter `unicode-script` crate gives you a `Script` enum whose `short_name()` method returns the ISO 15924 4-char code directly, and `as_iso15924_tag()` encodes it as a big-endian `u32`. Since ISO 15924 4-letter codes are **identical in casing** to OpenType tags for the vast majority of scripts, you can often just lowercase the short name to get the OT tag:[5]

```toml
[dependencies]
unicode-script = "0.5"
```

```rust
use unicode_script::Script;

fn iso15924_str_to_ot_tag(iso: &str) -> Option<String> {
    // unicode-script uses title-case ISO codes as the "short name"
    let script = Script::from_short_name(iso)?;
    // OT tags are lowercase; for most scripts this is correct.
    // The multi-version scripts (dev2, bng2 etc.) need special handling.
    Some(script.short_name().to_lowercase())
}

fn main() {
    println!("{:?}", iso15924_str_to_ot_tag("Cyrl")); // Some("cyrl")
    println!("{:?}", iso15924_str_to_ot_tag("Arab")); // Some("arab")
    println!("{:?}", iso15924_str_to_ot_tag("Latn")); // Some("latn")
}
```

For the v2 shaping tags (`dev2`, `bng2`, `gjr2`, etc.) you'll need a small static lookup table since those don't exist in ISO 15924 at all.[1]

***

## ISO 15924 → Unicode Codepoint (Range or Representative)

### What "codepoint" means here

Unicode's `Scripts.txt` (part of the UCD) maps every codepoint to an ISO 15924 script value. The inverse — getting codepoints *for* a script — gives you either a **range** or a **representative character**.[6][7]

### Option A: `unicode-script` — derive script of a char, scan for range

`unicode-script` exposes `UnicodeScript` trait on `char`, letting you go from codepoint → script. To go the other direction (script → codepoints), you iterate or use `as_iso15924_tag()` to identify, but the crate doesn't ship a reverse lookup. The simplest approach is to search `char::from_u32(0)..=char::from_u32(0x10FFFF)`:

```rust
use unicode_script::{Script, UnicodeScript};

fn first_codepoint_for_script(target: Script) -> Option<char> {
    (0u32..=0x10FFFF)
        .filter_map(char::from_u32)
        .find(|c| Script::from(*c) == target)
}

fn main() {
    let script = Script::from_short_name("Deva").unwrap();
    if let Some(ch) = first_codepoint_for_script(script) {
        println!("U+{:04X} {:?}", ch as u32, ch);
    }
}
```

This is exhaustive but fine for one-time table generation; cache results into a `HashMap<Script, char>` at startup.

### Option B: `codes-iso-15924` — range data from Wikimedia/UCD

The [`codes-iso-15924`](https://docs.rs/codes-iso-15924) crate provides `property_value_alias()` (the Unicode PVA like `"Devanagari"`) but does **not** embed codepoint ranges. For actual range data you have two good options:[8]

**1. Embed the Wikimedia unicode tab at build time.** The Wikimedia Commons `ISO15924/unicodes.tab` file maps each ISO code to Unicode hex ranges:[9]

```rust
// Build script or lazy_static — parse the tab file format:
// "Deva" → "0x0900 0x097F | 0xA8E0 0xA8FD"

use std::collections::HashMap;

fn parse_ranges(line: &str) -> Vec<(u32, u32)> {
    line.split('|')
        .filter_map(|part| {
            let mut nums = part.split_whitespace()
                .filter_map(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok());
            let start = nums.next()?;
            let end = nums.next().unwrap_or(start);
            Some((start, end))
        })
        .collect()
}
```

**2. Parse `Scripts.txt` from the UCD directly.** Download `https://www.unicode.org/Public/16.0.0/ucd/Scripts.txt`  and parse the semicolon-delimited ranges at build time with a `build.rs`, generating a `static` lookup table:[10]

```rust
// In build.rs, outputs src/script_ranges.rs
// Format: "0900..097F ; Devanagari"
for line in scripts_txt.lines() {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') { continue; }
    let (range_str, name) = line.split_once(';').unwrap();
    let name = name.trim().split_whitespace().next().unwrap();
    let (start_s, end_s) = range_str.trim()
        .split_once("..")
        .unwrap_or((range_str.trim(), range_str.trim()));
    let start = u32::from_str_radix(start_s, 16).unwrap();
    let end = u32::from_str_radix(end_s, 16).unwrap();
    // accumulate into HashMap<&str, Vec<(u32,u32)>>
}
```

***

## Handling Multi-OT-Tag Scripts

Some ISO 15924 codes map to **two** OpenType tags (the "v1" and "v2" shaping tags). You'll want an explicit override table in addition to `from_iso15924_tag`:

```rust
/// Returns (primary_ot_tag, optional_v2_ot_tag)
fn ot_tags_for_iso(iso: &str) -> (&'static str, Option<&'static str>) {
    match iso {
        "Deva" => ("deva", Some("dev2")),
        "Beng" => ("beng", Some("bng2")),
        "Gujr" => ("gujr", Some("gjr2")),
        "Guru" => ("guru", Some("gur2")),
        "Knda" => ("knda", Some("knd2")),
        "Mlym" => ("mlym", Some("mlm2")),
        "Orya" => ("ory2", Some("orya")),  // note: reversed priority
        "Taml" => ("taml", Some("tml2")),
        "Telu" => ("telu", Some("tel2")),
        _ => {
            // Fall back to lowercased ISO code
            // Use rustybuzz for verified mapping
            (Box::leak(iso.to_lowercase().into_boxed_str()), None)
        }
    }
}
```

The v2 tags are shaping-engine–specific and are not in ISO 15924, so this static table is the correct approach.[11][1]

***

## Recommended Cargo.toml

```toml
[dependencies]
# ISO 15924 ↔ OT script tag conversion (via HarfBuzz logic)
rustybuzz = "0.20"

# Script property of a char (UAX #24); also gives ISO 15924 short names
unicode-script = "0.5"

# ISO 15924 metadata (name, numeric code, PVA, Unicode version)
codes-iso-15924 = "0.1"
```

Use `rustybuzz` for the authoritative ISO→OT mapping (it inherits the complete HarfBuzz lookup table ), `unicode-script` for deriving a script from a `char` and scanning ranges, and either `codes-iso-15924` or a build.rs UCD parse for the codepoint range data.[12][2]

Sources
[1] Script tags (OpenType 1.8.4) - Typography | Microsoft Learn https://learn.microsoft.com/ru-ru/typography/opentype/otspec184/scripttags
[2] harfbuzz/rustybuzz - GitHub https://github.com/harfbuzz/rustybuzz
[3] Script in rustybuzz - Rust https://docs.rs/rustybuzz/latest/rustybuzz/struct.Script.html
[4] Tag in ttf_parser - Rust - Docs.rs https://docs.rs/ttf-parser/latest/ttf_parser/struct.Tag.html
[5] unicode_script - Rust - Docs.rs https://docs.rs/unicode-script
[6] Scripts - Unicode http://www.unicode.org/L2/L2000/00143-scripts.html
[7] PTR #24: Script Names http://www.unicode.org/reports/tr24/tr24-1.html
[8] codes_iso_15924 - Rust - Docs.rs https://docs.rs/codes-iso-15924/latest/codes_iso_15924/
[9] Data:ISO15924/unicodes.tab - Wikimedia Commons https://commons.wikimedia.org/wiki/Data:ISO15924/unicodes.tab
[10] Scripts.txt - Unicode https://www.unicode.org/Public/16.0.0/ucd/Scripts.txt
[11] Script tags (OpenType 1.7) - Typography | Microsoft Learn https://learn.microsoft.com/ru-ru/typography/opentype/otspec170/scripttags
[12] Script in rustybuzz::hb::common - Rust https://doc.servo.org/rustybuzz/hb/common/struct.Script.html
[13] Crate iso15924Copy item path https://docs.rs/iso15924/latest/iso15924/
[14] Script tags (OpenType 1.6) - Typography | Microsoft Learn https://learn.microsoft.com/en-us/typography/opentype/otspec160/scripttags
[15] ISO 15924 Registration Authority - Unicode https://www.unicode.org/iso15924/
[16] GitHub - rust-iso/rust_iso15924: ISO 15924, Codes for the representation of names of scripts, is an international standard defining codes for writing systems or scripts (a "set of graphic characters used for the written form of one or more languages"). Each script is given both a four-letter code and a numeric code. https://github.com/rust-iso/rust_iso15924
[17] Template:ISO 15924 alias/unicode-merged-into-script - Wikipedia https://en.wikipedia.org/wiki/Template:ISO_15924_alias/unicode-merged-into-script
[18] WikiProperty proposal/OpenType script tag https://www.wikidata.org/wiki/WikiProperty_proposal/OpenType_script_tag
[19] Meta table (OpenType 1.9.1) - Typography - Microsoft Learn https://learn.microsoft.com/en-us/typography/opentype/spec/meta
[20] python - Find out the unicode script of a character - Stack Overflow https://stackoverflow.com/questions/9868792/find-out-the-unicode-script-of-a-character
[21] Script tags (OpenType 1.8.2) - Typography - Microsoft Learn https://learn.microsoft.com/en-us/typography/opentype/otspec182/scripttags
[22] What is the state of Unicode support in Rust? - Reddit https://www.reddit.com/r/rust/comments/upgn1s/what_is_the_state_of_unicode_support_in_rust/
[23] Quickstart | ICU4X - Unicode https://icu4x.unicode.org/2_0/tutorials/quickstart/
[24] hb-common: HarfBuzz Manual - manpagez https://www.manpagez.com/html/harfbuzz/harfbuzz-2.6.3/harfbuzz-hb-common.php
[25] rustybuzz - Rust - Docs.rs https://docs.rs/rustybuzz/
[26] icu4x/tutorials/intro.md at main · unicode-org/icu4x https://github.com/unicode-org/icu4x/blob/main/tutorials/intro.md
[27] Only two character BCP47 language tags work, ISO-639-3 ... - GitHub https://github.com/harfbuzz/harfbuzzjs/issues/108
[28] rustybuzz - a complete harfbuzz's shaping algorithm port to safe Rust https://www.reddit.com/r/rust/comments/k76tjf/rustybuzz_a_complete_harfbuzzs_shaping_algorithm/
[29] icu4x/tutorials/quickstart.md at main · unicode-org/icu4x https://github.com/unicode-org/icu4x/blob/main/tutorials/quickstart.md
[30] Controls for OpenType - Wolthera.info https://wolthera.info/2025/03/controls-for-opentype/
[31] This is awesome, thanks to the authors of this, as well as all the ... https://news.ycombinator.com/item?id=35008956
[32] Rust Archiv https://www.credativ.de/en/tag/rust/
[33] ISO 15924 Alphabetical Code List - Unicode https://www.unicode.org/iso15924/iso15924-codes.html
[34] harfrust - Rust - Docs.rs https://docs.rs/harfrust
[35] ICU4X: What it can do and how to do it - credativ® https://www.credativ.de/en/blog/credativ-inside/icu4x-what-it-can-do-and-how-to-do-it/
[36] Script tags (OpenType 1.9.1) - Typography | Microsoft Learn https://learn.microsoft.com/en-us/typography/opentype/spec/scripttags
[37] ISO 15924 Script Codes https://search.r-project.org/CRAN/refmans/ISOcodes/html/ISO_15924.html
[38] GitHub - johnstonskj/rust-codes: A family of packages to provide standard codes in an independent yet structured manner https://github.com/johnstonskj/rust-codes
[39] ISO 15924 https://translatewiki.net/wiki/ISO_15924
[40] UnicodeBuffer in rustybuzz - Rust https://pop-os.github.io/libcosmic/rustybuzz/struct.UnicodeBuffer.html
[41] Template:ISO 15924 script codes and related Unicode data/sandbox - Wikipedia https://en.wikipedia.org/wiki/Template:ISO_15924_script_codes_and_related_Unicode_data/sandbox
[42] Template:ISO 15924 script codes and related Unicode data - Wikipedia https://en.wikipedia.org/wiki/Template:ISO_15924_script_codes_and_Unicode
[43] Struct UnicodeBufferCopy item path https://pop-os.github.io/cosmic-text/rustybuzz/struct.UnicodeBuffer.html
[44] Template:ISO 15924 script codes and related Unicode data - Wikipedia https://en.wikipedia.org/wiki/Template:ISO_15924_script_codes_and_related_Unicode_data
[45] Introducing UNIC: Unicode and Internationalization Crates for Rust https://users.rust-lang.org/t/introducing-unic-unicode-and-internationalization-crates-for-rust/11447
[46] Getting started with HarfBuzz https://harfbuzz.github.io/getting-started.html
[47] ISO 15924 Registration Authority http://unicode.org/iso15924/
[48] Supported Scripts - Unicode http://www.unicode.org/standard/supported.html
[49] harfbuzz/src/hb-unicode.hh at main · harfbuzz/harfbuzz https://github.com/harfbuzz/harfbuzz/blob/main/src/hb-unicode.hh
[50] GitHub - unicode-rs/unicode-script: Utilities for working with the Unicode Script and Script Extensions properties https://github.com/unicode-rs/unicode-script
[51] Characters by Script | Unicode - Charlotte Eiffel Lilith Buff https://charlottebuff.com/unicode/charts/scripts/
[52] HarfBuzz Manual: HarfBuzz Manual https://harfbuzz.github.io
[53] hb-unicode: HarfBuzz Manual https://harfbuzz.github.io/harfbuzz-hb-unicode.html
[54] Supported Scripts - Unicode https://www.unicode.org/standard/supported.html
[55] hb-common: HarfBuzz Manual https://harfbuzz.github.io/harfbuzz-hb-common.html
[56] Rust macro to create a byte-array backed struct - GitHub https://github.com/emeraldpay/byte-array-struct
[57] ISO 15924 Frequently Asked Questions - Unicode http://www.unicode.org/iso15924/faq.html
[58] How to use a struct as a u8 buffer? - Rust Users Forum https://users.rust-lang.org/t/how-to-use-a-struct-as-a-u8-buffer/106237
[59] ISO 15924 - Frequently Asked Questions (FAQ) https://www.unicode.org/iso15924/faq.html
[60] packbytes - Rust - Docs.rs https://docs.rs/packbytes
[61] convert from &[u8] to generic Sized struct : r/rust - Reddit https://www.reddit.com/r/rust/comments/dw2vb3/convert_from_u8_to_generic_sized_struct/
[62] UAX #24: Unicode Script Property http://www.unicode.org/reports/tr24/
[63] Dump Rust Struct or Enum Memory Representation as Bytes https://bennett.dev/rust/dump-struct-bytes/
[64] Read write a structure byte by byte - Programming - Arduino Forum https://forum.arduino.cc/t/read-write-a-structure-byte-by-byte/473508
[65] How to convert 'struct' to '&[u8]'? - Stack Overflow https://stackoverflow.com/questions/28127165/how-to-convert-struct-to-u8

====

To handle ISO 15924 script tags, OpenType script tags, and Unicode codepoints in Rust, you have to approach the problem in two parts.
Mapping ISO to OpenType involves dealing with font-shaping standards, while mapping ISO to Unicode involves querying Unicode Character Database (UCD) properties.
Here is how you can implement both mappings in your Rust application.
1. Mapping ISO Script Tags to OpenType Script Tags
The general rule of thumb is that an OpenType script tag is simply the lowercase version of the 4-letter ISO 15924 script tag (e.g., ISO Latn maps to OpenType latn).
However, OpenType introduced "version 2" tags for several complex scripts (mostly Indic scripts) to fix earlier shaping engine issues. For example, Devanagari (Deva) has an old tag deva and a new tag dev2. You usually want to map to the v2 tags if you are building modern text-shaping software.
You can write a function that handles the known exceptions and falls back to simply lowercasing the ISO tag.
/// Maps a 4-letter ISO 15924 script tag to a 4-byte OpenType script tag.
pub fn iso_to_opentype(iso_tag: &str) -> [u8; 4] {
    // Handle OpenType-specific exceptions (v2 scripts and aliases)
    match iso_tag {
        "Beng" => *b"bng2", // Bengali v2
        "Deva" => *b"dev2", // Devanagari v2
        "Gujr" => *b"gjr2", // Gujarati v2
        "Guru" => *b"gur2", // Gurmukhi v2
        "Knda" => *b"knd2", // Kannada v2
        "Mlym" => *b"mlm2", // Malayalam v2
        "Orya" => *b"ory2", // Odia v2
        "Taml" => *b"tml2", // Tamil v2
        "Telu" => *b"tel2", // Telugu v2
        "Mtei" => *b"mte1", // Meetei Mayek
        "Syrc" => *b"syrc", // Syriac
        // Add any other specific aliases your app needs here
        _ => {
            // Default behavior: lowercase the ISO tag and pad with spaces if needed
            let mut ot_tag = [b' '; 4];
            let bytes = iso_tag.as_bytes();
            for i in 0..bytes.len().min(4) {
                ot_tag[i] = bytes[i].to_ascii_lowercase();
            }
            ot_tag
        }
    }
}

fn main() {
    let latn_tag = iso_to_opentype("Latn");
    let deva_tag = iso_to_opentype("Deva");
    
    println!("Latn -> {:?}", std::str::from_utf8(&latn_tag).unwrap()); // latn
    println!("Deva -> {:?}", std::str::from_utf8(&deva_tag).unwrap()); // dev2
}

2. Mapping ISO Script Tags to Unicode Codepoints
To map ISO scripts to Unicode (or vice versa), the easiest and most robust method is to use the unicode-script crate. It provides an API mapped directly to the official Unicode properties.
Add this to your Cargo.toml:
[dependencies]
unicode-script = "0.5"

Because an ISO script tag corresponds to many different Unicode codepoints, "mapping" usually means you want to either:
1. Find out which ISO script a specific codepoint belongs to.
2. Check if a given codepoint belongs to a specific ISO script tag.
Here is how you do both using the crate:
use unicode_script::{Script, UnicodeScript};

fn main() {
    // --- Direction 1: Codepoint -> ISO Script Tag ---
    let my_char = 'Ω'; // Greek Omega
    let script = my_char.script(); 
    
    // `.short_name()` returns the standard 4-letter ISO 15924 tag
    println!("The ISO tag for '{}' is: {}", my_char, script.short_name()); 
    // Output: The ISO tag for 'Ω' is: Grek


    // --- Direction 2: ISO Script Tag -> Checking Codepoints ---
    let target_iso_tag = "Cyrl"; // Cyrillic
    
    // Parse the ISO string into a strong Script enum
    if let Some(target_script) = Script::from_short_name(target_iso_tag) {
        
        let test_char_1 = 'Д';
        let test_char_2 = 'A';
        
        // Check if the codepoints belong to the mapped script
        println!("Is '{}' Cyrillic? {}", test_char_1, test_char_1.script() == target_script); // true
        println!("Is '{}' Cyrillic? {}", test_char_2, test_char_2.script() == target_script); // false
        
    } else {
        println!("Unknown ISO 15924 tag: {}", target_iso_tag);
    }
}

===

