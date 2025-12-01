//! typg CLI (made by FontLab https://www.fontlab.com/)

use std::env;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum, ValueHint};
use regex::Regex;

use typg_core::output::{write_json_pretty, write_ndjson};
use typg_core::query::{parse_codepoint_list, parse_tag_list, Query};
use typg_core::search::{search, FontMatch, SearchOptions};

/// CLI entrypoint for typg.
#[derive(Debug, Parser)]
#[command(
    name = "typg",
    about = "Ultra-fast font search/discovery (made by FontLab https://www.fontlab.com/)"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Live search over filesystem paths (fontgrep parity)
    Find(FindArgs),
}

#[derive(Debug, Args)]
struct FindArgs {
    /// Paths to search (directories or files)
    #[arg(
        value_hint = ValueHint::DirPath,
        required_unless_present_any = ["system_fonts", "stdin_paths"]
    )]
    paths: Vec<PathBuf>,

    /// Read newline-delimited paths from STDIN
    #[arg(long = "stdin-paths", action = ArgAction::SetTrue)]
    stdin_paths: bool,

    /// Include common system font directories automatically
    #[arg(long = "system-fonts", action = ArgAction::SetTrue)]
    system_fonts: bool,

    /// Require fonts to define these axis tags
    #[arg(short = 'a', long = "axes", value_delimiter = ',', value_hint = ValueHint::Other)]
    axes: Vec<String>,

    /// Require fonts to define these OpenType feature tags
    #[arg(short = 'f', long = "features", value_delimiter = ',', value_hint = ValueHint::Other)]
    features: Vec<String>,

    /// Require fonts to cover these script tags
    #[arg(short = 's', long = "scripts", value_delimiter = ',', value_hint = ValueHint::Other)]
    scripts: Vec<String>,

    /// Require fonts to contain these table tags
    #[arg(short = 'T', long = "tables", value_delimiter = ',', value_hint = ValueHint::Other)]
    tables: Vec<String>,

    /// Regex patterns that must match at least one font name
    #[arg(short = 'n', long = "name", value_hint = ValueHint::Other)]
    name_patterns: Vec<String>,

    /// Unicode codepoints or ranges (e.g. U+0041-U+0044,B)
    #[arg(short = 'u', long = "codepoints", value_delimiter = ',', value_hint = ValueHint::Other)]
    codepoints: Vec<String>,

    /// Require fonts to cover this text sample
    #[arg(short = 't', long = "text")]
    text: Option<String>,

    /// Only include variable fonts
    #[arg(short = 'v', long = "variable", action = ArgAction::SetTrue)]
    variable: bool,

    /// Follow symlinks while walking paths
    #[arg(long = "follow-symlinks", action = ArgAction::SetTrue)]
    follow_symlinks: bool,

    /// Emit a single JSON array
    #[arg(long = "json", action = ArgAction::SetTrue, conflicts_with = "ndjson")]
    json: bool,

    /// Emit newline-delimited JSON
    #[arg(long = "ndjson", action = ArgAction::SetTrue)]
    ndjson: bool,

    /// Format output as padded columns
    #[arg(long = "columns", action = ArgAction::SetTrue)]
    columns: bool,

    /// Control colorized output (auto|always|never)
    #[arg(long = "color", default_value_t = ColorChoice::Auto, value_enum)]
    color: ColorChoice,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum ColorChoice {
    Auto,
    Always,
    Never,
}

/// Parse CLI args and execute the selected command.
pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Find(args) => run_find(args),
    }
}

fn run_find(args: FindArgs) -> Result<()> {
    let stdin = io::stdin();
    let paths = gather_paths(
        &args.paths,
        args.stdin_paths,
        args.system_fonts,
        stdin.lock(),
    )?;
    let query = build_query(&args)?;
    let opts = SearchOptions {
        follow_symlinks: args.follow_symlinks,
    };

    let matches = search(&paths, &query, &opts)?;

    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let use_color = match args.color {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => handle.is_terminal(),
    };

    if args.ndjson {
        write_ndjson(&matches, &mut handle)?;
    } else if args.json {
        write_json_pretty(&matches, &mut handle)?;
    } else if args.columns {
        write_columns(&matches, &mut handle, use_color)?;
    } else {
        write_plain(&matches, &mut handle, use_color)?;
    }

    Ok(())
}

fn build_query(args: &FindArgs) -> Result<Query> {
    let axes = parse_tag_list(&args.axes)?;
    let features = parse_tag_list(&args.features)?;
    let scripts = parse_tag_list(&args.scripts)?;
    let tables = parse_tag_list(&args.tables)?;
    let name_patterns = compile_patterns(&args.name_patterns)?;
    let mut codepoints = parse_codepoints(&args.codepoints)?;

    if let Some(text) = &args.text {
        codepoints.extend(text.chars());
    }

    dedup_chars(&mut codepoints);

    Ok(Query::new()
        .with_axes(axes)
        .with_features(features)
        .with_scripts(scripts)
        .with_tables(tables)
        .with_name_patterns(name_patterns)
        .with_codepoints(codepoints)
        .require_variable(args.variable))
}

fn dedup_chars(cps: &mut Vec<char>) {
    cps.sort();
    cps.dedup();
}

fn compile_patterns(patterns: &[String]) -> Result<Vec<Regex>> {
    patterns
        .iter()
        .map(|p| Regex::new(p).with_context(|| format!("invalid regex: {p}")))
        .collect()
}

fn parse_codepoints(raw: &[String]) -> Result<Vec<char>> {
    let mut cps = Vec::new();
    for chunk in raw {
        cps.extend(parse_codepoint_list(chunk)?);
    }
    Ok(cps)
}

fn gather_paths(
    raw_paths: &[PathBuf],
    read_stdin: bool,
    include_system: bool,
    mut stdin: impl BufRead,
) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    if read_stdin {
        paths.extend(read_paths_from(&mut stdin)?);
    }

    for path in raw_paths {
        if path == Path::new("-") {
            paths.extend(read_paths_from(&mut stdin)?);
        } else {
            paths.push(path.clone());
        }
    }

    if include_system {
        paths.extend(system_font_roots()?);
    }

    if paths.is_empty() {
        return Err(anyhow!("no search paths provided"));
    }

    Ok(paths)
}

fn read_paths_from(reader: &mut impl BufRead) -> Result<Vec<PathBuf>> {
    let mut buf = String::new();
    let mut paths = Vec::new();

    loop {
        buf.clear();
        let read = reader.read_line(&mut buf)?;
        if read == 0 {
            break;
        }

        let trimmed = buf.trim();
        if !trimmed.is_empty() {
            paths.push(PathBuf::from(trimmed));
        }
    }

    Ok(paths)
}

fn system_font_roots() -> Result<Vec<PathBuf>> {
    if let Ok(raw) = env::var("TYPOG_SYSTEM_FONT_DIRS") {
        let mut overrides: Vec<PathBuf> = raw
            .split([':', ';'])
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .collect();

        overrides.sort();
        overrides.dedup();

        return if overrides.is_empty() {
            Err(anyhow!("TYPOG_SYSTEM_FONT_DIRS is set but no paths exist"))
        } else {
            Ok(overrides)
        };
    }

    let mut candidates: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "macos")]
    {
        candidates.push(PathBuf::from("/System/Library/Fonts"));
        candidates.push(PathBuf::from("/Library/Fonts"));
        if let Some(home) = env::var_os("HOME") {
            candidates.push(PathBuf::from(home).join("Library/Fonts"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        candidates.push(PathBuf::from("/usr/share/fonts"));
        candidates.push(PathBuf::from("/usr/local/share/fonts"));
        if let Some(home) = env::var_os("HOME") {
            candidates.push(PathBuf::from(home).join(".local/share/fonts"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(system_root) = env::var_os("SYSTEMROOT") {
            candidates.push(PathBuf::from(system_root).join("Fonts"));
        }
        if let Some(local_appdata) = env::var_os("LOCALAPPDATA") {
            candidates.push(PathBuf::from(local_appdata).join("Microsoft/Windows/Fonts"));
        }
    }

    candidates.retain(|p| p.exists());
    candidates.sort();
    candidates.dedup();

    if candidates.is_empty() {
        return Err(anyhow!(
            "no system font directories found for this platform"
        ));
    }

    Ok(candidates)
}

fn write_plain(matches: &[FontMatch], mut w: impl Write, color: bool) -> Result<()> {
    for item in matches {
        let rendered = render_path(item, color);
        writeln!(w, "{rendered}")?;
    }
    Ok(())
}

fn write_columns(matches: &[FontMatch], mut w: impl Write, color: bool) -> Result<()> {
    let mut rows: Vec<(String, String, String)> = matches
        .iter()
        .map(|m| {
            let path = path_with_index(m);
            let name = m
                .metadata
                .names
                .first()
                .cloned()
                .unwrap_or_else(|| "(unnamed)".to_string());

            let tags = format!(
                "axes:{:<2} feats:{:<2} scripts:{:<2} tables:{:<2}{}",
                m.metadata.axis_tags.len(),
                m.metadata.feature_tags.len(),
                m.metadata.script_tags.len(),
                m.metadata.table_tags.len(),
                if m.metadata.is_variable { " var" } else { "" },
            );

            (path, name, tags)
        })
        .collect();

    let path_width = rows
        .iter()
        .map(|r| r.0.len())
        .max()
        .unwrap_or(0)
        .clamp(0, 120);
    let name_width = rows
        .iter()
        .map(|r| r.1.len())
        .max()
        .unwrap_or(0)
        .clamp(0, 80);

    for (path, name, tags) in rows.drain(..) {
        let padded_path = format!("{:<path_width$}", path);
        let padded_name = format!("{:<name_width$}", name);
        let rendered_path = apply_color(&padded_path, color, AnsiColor::Cyan);
        let rendered_name = apply_color(&padded_name, color, AnsiColor::Yellow);
        let rendered_tags = apply_color(&tags, color, AnsiColor::Green);

        writeln!(w, "{rendered_path}  {rendered_name}  {rendered_tags}")?;
    }

    Ok(())
}

#[derive(Copy, Clone)]
enum AnsiColor {
    Cyan,
    Yellow,
    Green,
}

fn apply_color(text: &str, color: bool, code: AnsiColor) -> String {
    if !color {
        return text.to_string();
    }

    let code_str = match code {
        AnsiColor::Cyan => "36",
        AnsiColor::Yellow => "33",
        AnsiColor::Green => "32",
    };

    format!("\u{1b}[{}m{}\u{1b}[0m", code_str, text)
}

fn render_path(item: &FontMatch, color: bool) -> String {
    let rendered = path_with_index(item);
    apply_color(&rendered, color, AnsiColor::Cyan)
}

fn path_with_index(item: &FontMatch) -> String {
    if let Some(idx) = item.metadata.ttc_index {
        format!("{}#{}", item.path.display(), idx)
    } else {
        item.path.display().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use std::io::Cursor;
    use tempfile::tempdir;
    use typg_core::search::FontMetadata;
    use typg_core::tags::tag4;

    fn metadata_with(name: &str, axis: Option<&str>, ttc: Option<u32>) -> FontMetadata {
        FontMetadata {
            path: PathBuf::from(format!("/fonts/{}.ttf", name.to_lowercase())),
            names: vec![name.to_string()],
            axis_tags: axis.into_iter().map(|t| tag4(t).expect("tag")).collect(),
            feature_tags: Vec::new(),
            script_tags: Vec::new(),
            table_tags: Vec::new(),
            codepoints: vec!['A'],
            is_variable: axis.is_some(),
            ttc_index: ttc,
        }
    }

    #[test]
    fn parses_find_args_into_query() {
        let cli = Cli::try_parse_from([
            "typg", "find", "-a", "wght", "-f", "liga", "-s", "latn", "-T", "GPOS", "-n", "Mono",
            "-u", "U+0041", "-v", "--json", "/fonts",
        ])
        .expect("parse cli");

        let Command::Find(args) = cli.command;

        let query = build_query(&args).expect("build query");
        assert!(args.json);
        assert!(!args.ndjson);

        let mut matching = metadata_with("Mono", Some("wght"), None);
        matching.feature_tags = vec![tag4("liga").unwrap()];
        matching.script_tags = vec![tag4("latn").unwrap()];
        matching.table_tags = vec![tag4("GPOS").unwrap()];
        assert!(query.matches(&matching));

        let non_matching = metadata_with("Sans", None, None);
        assert!(!query.matches(&non_matching));
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
            FontMatch {
                path: PathBuf::from("/fonts/A.ttf"),
                metadata: metadata_with("A", None, None),
            },
            FontMatch {
                path: PathBuf::from("/fonts/B.ttc"),
                metadata: metadata_with("B", None, Some(2)),
            },
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
        meta.codepoints = vec!['A', 'B'];
        assert!(query.matches(&meta));
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
            FontMatch {
                path: PathBuf::from("/fonts/A.ttf"),
                metadata: metadata_with("Alpha", Some("wght"), None),
            },
            FontMatch {
                path: PathBuf::from("/fonts/B.ttf"),
                metadata: metadata_with("Beta", None, None),
            },
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
        let matches = vec![FontMatch {
            path: PathBuf::from("/fonts/A.ttf"),
            metadata: metadata_with("Alpha", None, None),
        }];

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
}
