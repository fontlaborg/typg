use proptest::prelude::*;

use typg_core::query::parse_codepoint_list;

#[test]
fn parses_single_codepoint_and_range() {
    let cps = parse_codepoint_list("A,U+0042-U+0044").expect("parse");
    assert_eq!(cps, vec!['A', 'B', 'C', 'D']);
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
