use super::*;
use clap::CommandFactory;
use std::io::Cursor;
use tempfile::tempdir;
use typg_core::search::{TypgFontFaceMatch, TypgFontFaceMeta, TypgFontSource};
use typg_core::tags::tag4;

fn metadata_with(name: &str, axis: Option<&str>, ttc: Option<u32>) -> TypgFontFaceMatch {
    let ext = if ttc.is_some() { "ttc" } else { "ttf" };
    TypgFontFaceMatch {
        source: TypgFontSource {
            path: PathBuf::from(format!("/fonts/{}.{}", name, ext)),
            ttc_index: ttc,
        },
        metadata: TypgFontFaceMeta {
            names: vec![name.to_string()],
            axis_tags: axis.into_iter().map(|t| tag4(t).expect("tag")).collect(),
            feature_tags: Vec::new(),
            script_tags: Vec::new(),
            table_tags: Vec::new(),
            codepoints: vec!['A'],
            is_variable: axis.is_some(),
        },
    }
}

#[test]
fn parses_find_args_into_query() {
    let cli = Cli::try_parse_from([
        "typg", "find", "-a", "wght", "-f", "liga", "-s", "latn", "-T", "GPOS", "-n", "Mono", "-u",
        "U+0041", "-v", "--json", "/fonts",
    ])
    .expect("parse cli");

    let Command::Find(args) = cli.command;

    let query = build_query(&args).expect("build query");
    assert!(args.json);
    assert!(!args.ndjson);

    let mut matching = metadata_with("Mono", Some("wght"), None);
    matching.metadata.feature_tags = vec![tag4("liga").unwrap()];
    matching.metadata.script_tags = vec![tag4("latn").unwrap()];
    matching.metadata.table_tags = vec![tag4("GPOS").unwrap()];
    assert!(query.matches(&matching.metadata));

    let non_matching = metadata_with("Sans", None, None);
    assert!(!query.matches(&non_matching.metadata));
}

#[test]
fn json_and_ndjson_conflict() {
    let parse = Cli::try_parse_from(["typg", "find", "--json", "--ndjson", "/fonts"]);
    assert!(parse.is_err());
}

#[test]
fn invalid_regex_returns_error() {
    let args = FindArgs {
        paths: vec![PathBuf::from("/fonts")],
        axes: Vec::new(),
        features: Vec::new(),
        scripts: Vec::new(),
        tables: Vec::new(),
        name_patterns: vec!["(".to_string()],
        codepoints: Vec::new(),
        text: None,
        variable: false,
        follow_symlinks: false,
        stdin_paths: false,
        system_fonts: false,
        json: false,
        ndjson: false,
        columns: false,
        color: ColorChoice::Auto,
    };

    let built = build_query(&args);
    assert!(built.is_err());
}

#[test]
fn writes_plain_with_ttc_suffix() {
    let matches = vec![
        metadata_with("A", None, None),
        metadata_with("B", None, Some(2)),
    ];

    let mut buf = Cursor::new(Vec::new());
    write_plain(&matches, &mut buf, false).expect("write");

    let output = String::from_utf8(buf.into_inner()).expect("utf8");
    assert!(output.contains("/fonts/A.ttf"));
    assert!(output.contains("/fonts/B.ttc#2"));
}

#[test]
fn text_flag_merges_into_codepoints() {
    let cli = Cli::try_parse_from(["typg", "find", "-u", "U+0041", "-t", "B", "/fonts"])
        .expect("parse cli");

    let Command::Find(args) = cli.command;
    let query = build_query(&args).expect("build");

    let mut meta = metadata_with("AB", None, None);
    meta.metadata.codepoints = vec!['A', 'B'];
    assert!(query.matches(&meta.metadata));
}

#[test]
fn gathers_paths_from_stdin_when_flagged() {
    let mut stdin = Cursor::new(b"/fonts/A\n/fonts/B\n".to_vec());
    let paths = gather_paths(&[], true, false, &mut stdin).expect("paths");

    assert_eq!(
        paths,
        vec![PathBuf::from("/fonts/A"), PathBuf::from("/fonts/B")]
    );
}

#[test]
fn dash_placeholder_reads_stdin_and_merges_other_paths() {
    let mut stdin = Cursor::new(b"/fonts/A\n".to_vec());
    let paths = gather_paths(
        &[PathBuf::from("-"), PathBuf::from("/fonts/B")],
        false,
        false,
        &mut stdin,
    )
    .expect("paths");

    assert_eq!(
        paths,
        vec![PathBuf::from("/fonts/A"), PathBuf::from("/fonts/B")]
    );
}

#[test]
fn system_font_roots_uses_override_env() {
    let tmp = tempdir().expect("tempdir");
    let font_dir = tmp.path().join("fonts");
    std::fs::create_dir_all(&font_dir).expect("mkdir");

    env::set_var("TYPOG_SYSTEM_FONT_DIRS", font_dir.display().to_string());
    let roots = system_font_roots().expect("roots");
    env::remove_var("TYPOG_SYSTEM_FONT_DIRS");

    assert_eq!(roots, vec![font_dir]);
}

#[test]
fn columns_align_names() {
    let matches = vec![
        metadata_with("Alpha", Some("wght"), None),
        metadata_with("Beta", None, None),
    ];

    let mut buf = Cursor::new(Vec::new());
    write_columns(&matches, &mut buf, false).expect("write");

    let output = String::from_utf8(buf.into_inner()).expect("utf8");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 2);
    let alpha_pos = lines[0].find("Alpha").expect("alpha");
    let beta_pos = lines[1].find("Beta").expect("beta");
    assert_eq!(alpha_pos, beta_pos);
}

#[test]
fn color_choice_is_applied() {
    let matches = vec![metadata_with("Alpha", None, None)];

    let mut buf = Cursor::new(Vec::new());
    write_plain(&matches, &mut buf, true).expect("write");

    let output = String::from_utf8(buf.into_inner()).expect("utf8");
    assert!(output.contains("\u{1b}["));
}

#[test]
fn parses_color_and_columns_flags() {
    let cli = Cli::try_parse_from(["typg", "find", "--columns", "--color", "always", "/fonts"])
        .expect("parse cli");

    let Command::Find(args) = cli.command;
    assert!(args.columns);
    assert_eq!(args.color, ColorChoice::Always);
}

#[test]
fn help_output_includes_new_flags() {
    let mut root = Cli::command();
    let find = root
        .find_subcommand_mut("find")
        .expect("find command present");
    let help = find.render_long_help().to_string();
    assert!(help.contains("--columns"));
    assert!(help.contains("--color <COLOR>"));
}
