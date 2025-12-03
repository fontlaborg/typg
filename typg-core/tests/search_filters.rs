use typg_core::query::Query;
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
) -> TypgFontFaceMeta {
    TypgFontFaceMeta {
        names: vec![name.to_string()],
        axis_tags: axes.iter().map(|t| tag4(t).unwrap()).collect(),
        feature_tags: features.iter().map(|t| tag4(t).unwrap()).collect(),
        script_tags: scripts.iter().map(|t| tag4(t).unwrap()).collect(),
        table_tags: tables.iter().map(|t| tag4(t).unwrap()).collect(),
        codepoints: codepoints.to_vec(),
        is_variable: variable,
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
    );

    assert!(query.matches(&meta));
}

#[test]
fn fails_when_missing_codepoint() {
    let query = Query::new().with_codepoints(vec!['Ω']);
    let meta = metadata_with("File", &[], &[], &[], &[], &['A'], false);

    assert!(!query.matches(&meta));
}

#[test]
fn fails_when_missing_table() {
    let query = Query::new().with_tables(vec![tag4("GSUB").unwrap()]);
    let meta = metadata_with("File", &[], &[], &[], &["GPOS"], &[], false);

    assert!(!query.matches(&meta));
}

#[test]
fn name_regex_must_match_any_name() {
    let query = Query::new().with_name_patterns(vec![regex::Regex::new("Mono").unwrap()]);
    let mut meta = metadata_with("Sans", &[], &[], &[], &[], &[], false);
    meta.names.push("Mono Sans".to_string());

    assert!(query.matches(&meta));
}

#[test]
fn variable_filter_blocks_static_fonts() {
    let query = Query::new().require_variable(true);
    let meta = metadata_with("File", &[], &[], &[], &[], &[], false);

    assert!(!query.matches(&meta));
}
