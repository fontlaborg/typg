//! typg CLI (made by FontLab https://www.fontlab.com/)

use std::env;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum, ValueHint};
use regex::Regex;

use typg_core::output::{write_json_pretty, write_ndjson};
use typg_core::query::{parse_codepoint_list, parse_tag_list, Query};
use typg_core::search::{search, SearchOptions, TypgFontFaceMatch};

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

fn write_plain(matches: &[TypgFontFaceMatch], mut w: impl Write, color: bool) -> Result<()> {
    for item in matches {
        let rendered = render_path(item, color);
        writeln!(w, "{rendered}")?;
    }
    Ok(())
}

fn write_columns(matches: &[TypgFontFaceMatch], mut w: impl Write, color: bool) -> Result<()> {
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

fn render_path(item: &TypgFontFaceMatch, color: bool) -> String {
    let rendered = path_with_index(item);
    apply_color(&rendered, color, AnsiColor::Cyan)
}

fn path_with_index(item: &TypgFontFaceMatch) -> String {
    if let Some(idx) = item.source.ttc_index {
        format!("{}#{}", item.source.path.display(), idx)
    } else {
        item.source.path.display().to_string()
    }
}

#[cfg(test)]
mod tests;
