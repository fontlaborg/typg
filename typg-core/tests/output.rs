use typg_core::output::{write_json_pretty, write_ndjson};
use typg_core::search::{FontMatch, FontMetadata};
use typg_core::tags::tag4;

#[test]
fn writes_ndjson_one_object_per_line() {
    let fonts = sample_fonts();
    let mut buf = Vec::new();

    write_ndjson(&fonts, &mut buf).expect("write");
    let text = String::from_utf8(buf).expect("utf8");
    let lines: Vec<&str> = text.trim_end().split('\n').collect();

    assert_eq!(lines.len(), 2);
    for line in lines {
        serde_json::from_str::<serde_json::Value>(line).expect("valid json line");
    }
}

#[test]
fn writes_pretty_json_array() {
    let fonts = sample_fonts();
    let mut buf = Vec::new();

    write_json_pretty(&fonts, &mut buf).expect("write");
    let text = String::from_utf8(buf).expect("utf8");

    let parsed: serde_json::Value = serde_json::from_str(&text).expect("json array");
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 2);
}

fn sample_fonts() -> Vec<FontMatch> {
    vec![
        FontMatch {
            path: "fonts/A.ttf".into(),
            metadata: FontMetadata {
                path: "fonts/A.ttf".into(),
                names: vec!["Alpha".into()],
                axis_tags: vec![tag4("wght").unwrap()],
                feature_tags: vec![],
                script_tags: vec![],
                table_tags: vec![tag4("fvar").unwrap()],
                codepoints: vec!['A', 'B'],
                is_variable: true,
                ttc_index: None,
            },
        },
        FontMatch {
            path: "fonts/B.otf".into(),
            metadata: FontMetadata {
                path: "fonts/B.otf".into(),
                names: vec!["Beta".into()],
                axis_tags: vec![],
                feature_tags: vec![],
                script_tags: vec![],
                table_tags: vec![],
                codepoints: vec!['A'],
                is_variable: false,
                ttc_index: Some(1),
            },
        },
    ]
}
