use proptest::prelude::*;

use typg_core::query::{parse_codepoint_list, parse_family_class, parse_u16_range};

#[test]
fn parses_single_codepoint_and_range() {
    let cps = parse_codepoint_list("A,U+0042-U+0044").expect("parse");
    assert_eq!(cps, vec!['A', 'B', 'C', 'D']);
}

#[test]
fn parses_u16_range_and_value() {
    let single = parse_u16_range("400").expect("single weight");
    assert!(single.contains(&400));
    assert!(!single.contains(&401));

    let range = parse_u16_range("300-500").expect("range");
    assert!(range.contains(&300));
    assert!(range.contains(&450));
    assert!(range.contains(&500));
}

proptest! {
    #[test]
    fn parses_inclusive_ranges(start in 0x0041u32..0x007A, end in 0x0041u32..0x007A) {
        let (lo, hi) = if start <= end { (start, end) } else { (end, start) };
        // Skip invalid Unicode scalars (surrogates) by constraining ranges above
        let range = format!("U+{lo:04X}-U+{hi:04X}");
        let parsed = parse_codepoint_list(&range).expect("parse range");

        let expected_len = (hi - lo + 1) as usize;
        prop_assert_eq!(parsed.len(), expected_len);
        prop_assert_eq!(parsed.first().copied(), char::from_u32(lo));
        prop_assert_eq!(parsed.last().copied(), char::from_u32(hi));
    }
}

#[test]
fn parses_family_class_major_and_subclass() {
    let sans = parse_family_class("8").expect("parse sans major");
    assert_eq!(sans.major, 8);
    assert_eq!(sans.subclass, None);

    let subclass = parse_family_class("8.11").expect("parse subclass");
    assert_eq!(subclass.major, 8);
    assert_eq!(subclass.subclass, Some(11));

    let hex = parse_family_class("0x080B").expect("parse hex value");
    assert_eq!(hex.major, 8);
    assert_eq!(hex.subclass, Some(11));
}

#[test]
fn parses_family_class_names() {
    let oldstyle = parse_family_class("oldstyle").expect("parse name");
    assert_eq!(oldstyle.major, 1);
    assert_eq!(oldstyle.subclass, None);

    let sans = parse_family_class("sans-serif").expect("parse sans");
    assert_eq!(sans.major, 8);
}
