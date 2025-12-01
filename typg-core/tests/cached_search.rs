use std::path::PathBuf;

use typg_core::query::Query;
use typg_core::search::{filter_cached, FontMetadata};
use typg_core::tags::tag4;

fn metadata_with(name: &str, axis: Option<&str>, ttc_index: Option<u32>) -> FontMetadata {
    FontMetadata {
        path: PathBuf::from(format!("/fonts/{}.ttf", name.to_lowercase())),
        names: vec![name.to_string()],
        axis_tags: axis.into_iter().map(|t| tag4(t).expect("tag")).collect(),
        feature_tags: Vec::new(),
        script_tags: Vec::new(),
        table_tags: Vec::new(),
        codepoints: vec!['A'],
        is_variable: axis.is_some(),
        ttc_index,
    }
}

#[test]
fn filters_cached_metadata_without_io() {
    let entries = vec![
        metadata_with("Sans", Some("wght"), None),
        metadata_with("Mono", None, Some(1)),
    ];

    let query = Query::new()
        .with_axes(vec![tag4("wght").unwrap()])
        .require_variable(true)
        .with_codepoints(vec!['A']);

    let matches = filter_cached(&entries, &query);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].metadata.names[0], "Sans");
    assert!(matches[0].metadata.is_variable);
}

#[test]
fn sorts_by_path_and_ttc_index() {
    let entries = vec![
        metadata_with("B", None, Some(2)),
        metadata_with("A", None, None),
        metadata_with("B", None, Some(1)),
    ];

    let query = Query::new();
    let matches = filter_cached(&entries, &query);

    let names: Vec<(String, Option<u32>)> = matches
        .iter()
        .map(|m| (m.metadata.names[0].clone(), m.metadata.ttc_index))
        .collect();

    assert_eq!(
        names,
        vec![
            ("A".to_string(), None),
            ("B".to_string(), Some(1)),
            ("B".to_string(), Some(2)),
        ]
    );
}
