use typg_core::query::{FamilyClassFilter, Query};
use typg_core::search::TypgFontFaceMeta;
use typg_core::tags::tag4;

fn metadata_with(
    name: &str,
    axes: &[&str],
    features: &[&str],
    scripts: &[&str],
    tables: &[&str],
    codepoints: &[char],
    variable: bool,
    weight_class: Option<u16>,
    width_class: Option<u16>,
    family_class: Option<(u8, u8)>,
) -> TypgFontFaceMeta {
    TypgFontFaceMeta {
        names: vec![name.to_string()],
        axis_tags: axes.iter().map(|t| tag4(t).unwrap()).collect(),
        feature_tags: features.iter().map(|t| tag4(t).unwrap()).collect(),
        script_tags: scripts.iter().map(|t| tag4(t).unwrap()).collect(),
        table_tags: tables.iter().map(|t| tag4(t).unwrap()).collect(),
        codepoints: codepoints.to_vec(),
        is_variable: variable,
        weight_class,
        width_class,
        family_class,
    }
}

#[test]
fn matches_when_all_filters_satisfied() {
    let query = Query::new()
        .with_axes(vec![tag4("wght").unwrap()])
        .with_features(vec![tag4("liga").unwrap()])
        .with_scripts(vec![tag4("latn").unwrap()])
        .with_tables(vec![tag4("GPOS").unwrap()])
        .with_codepoints(vec!['A'])
        .require_variable(true)
        .with_name_patterns(vec![regex::Regex::new("Pro").unwrap()]);

    let meta = metadata_with(
        "ProFont",
        &["wght"],
        &["liga"],
        &["latn"],
        &["GPOS", "GSUB"],
        &['A', 'B'],
        true,
        Some(400),
        Some(5),
        Some((8, 0)),
    );

    assert!(query.matches(&meta));
}

#[test]
fn fails_when_missing_codepoint() {
    let query = Query::new().with_codepoints(vec!['Ω']);
    let meta = metadata_with("File", &[], &[], &[], &[], &['A'], false, None, None, None);

    assert!(!query.matches(&meta));
}

#[test]
fn fails_when_missing_table() {
    let query = Query::new().with_tables(vec![tag4("GSUB").unwrap()]);
    let meta = metadata_with(
        "File",
        &[],
        &[],
        &[],
        &["GPOS"],
        &[],
        false,
        None,
        None,
        None,
    );

    assert!(!query.matches(&meta));
}

#[test]
fn name_regex_must_match_any_name() {
    let query = Query::new().with_name_patterns(vec![regex::Regex::new("Mono").unwrap()]);
    let mut meta = metadata_with("Sans", &[], &[], &[], &[], &[], false, None, None, None);
    meta.names.push("Mono Sans".to_string());

    assert!(query.matches(&meta));
}

#[test]
fn variable_filter_blocks_static_fonts() {
    let query = Query::new().require_variable(true);
    let meta = metadata_with("File", &[], &[], &[], &[], &[], false, None, None, None);

    assert!(!query.matches(&meta));
}

#[test]
fn weight_range_filters_os2_value() {
    let query = Query::new().with_weight_range(Some(300..=500));
    let matching = metadata_with(
        "Book",
        &[],
        &[],
        &[],
        &[],
        &[],
        false,
        Some(400),
        None,
        None,
    );
    assert!(query.matches(&matching));

    let missing = metadata_with("Unknown", &[], &[], &[], &[], &[], false, None, None, None);
    assert!(!query.matches(&missing));

    let out_of_range = metadata_with(
        "Black",
        &[],
        &[],
        &[],
        &[],
        &[],
        false,
        Some(800),
        None,
        None,
    );
    assert!(!query.matches(&out_of_range));
}

#[test]
fn width_range_filters_os2_value() {
    let query = Query::new().with_width_range(Some(3..=5));

    let matching = metadata_with(
        "Condensed",
        &[],
        &[],
        &[],
        &[],
        &[],
        false,
        None,
        Some(4),
        None,
    );
    assert!(query.matches(&matching));

    let out = metadata_with("Wide", &[], &[], &[], &[], &[], false, None, Some(7), None);
    assert!(!query.matches(&out));
}

#[test]
fn family_class_filters_on_major_only() {
    let query = Query::new().with_family_class(Some(FamilyClassFilter {
        major: 8,
        subclass: None,
    }));

    let sans = metadata_with(
        "Sans",
        &[],
        &[],
        &[],
        &[],
        &[],
        false,
        None,
        None,
        Some((8, 11)),
    );
    assert!(query.matches(&sans));

    let serif = metadata_with(
        "Serif",
        &[],
        &[],
        &[],
        &[],
        &[],
        false,
        None,
        None,
        Some((1, 0)),
    );
    assert!(!query.matches(&serif));
}

#[test]
fn family_class_filters_on_subclass_when_present() {
    let query = Query::new().with_family_class(Some(FamilyClassFilter {
        major: 8,
        subclass: Some(11),
    }));

    let matching = metadata_with(
        "Sans",
        &[],
        &[],
        &[],
        &[],
        &[],
        false,
        None,
        None,
        Some((8, 11)),
    );
    assert!(query.matches(&matching));

    let different_subclass = metadata_with(
        "Sans",
        &[],
        &[],
        &[],
        &[],
        &[],
        false,
        None,
        None,
        Some((8, 0)),
    );
    assert!(!query.matches(&different_subclass));
}
