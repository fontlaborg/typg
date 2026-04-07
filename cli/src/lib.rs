//! typg CLI — font search and cache management.
//!
//! Made by FontLab https://www.fontlab.com/

mod server;

use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, BufWriter, IsTerminal, Write};
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum, ValueHint};
use regex::Regex;
use serde_json::Deserializer;
use tokio::runtime::Builder;

use typg_core::output::{write_json_pretty, write_ndjson};
use typg_core::query::{
    parse_codepoint_list, parse_family_class, parse_tag_list, parse_u16_range, FamilyClassFilter,
    Query,
};
use typg_core::search::{
    filter_cached, search, search_streaming, SearchOptions, TypgFontFaceMatch,
};

#[cfg(feature = "hpindex")]
use typg_core::index::FontIndex;

/// Top-level CLI definition.
#[derive(Debug, Parser)]
#[command(
    name = "typg",
    version,
    about = "Fast font search (made by FontLab https://www.fontlab.com/)"
)]
pub struct Cli {
    /// Suppress informational messages
    #[arg(short = 'q', long = "quiet", global = true, action = ArgAction::SetTrue)]
    quiet: bool,

    #[command(subcommand)]
    command: Command,
}

/// Available subcommands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Search directories for fonts matching a query
    Find(Box<FindArgs>),

    /// Manage the font metadata cache
    #[command(subcommand)]
    Cache(CacheCommand),

    /// Start an HTTP search server
    Serve(ServeArgs),
}

/// Cache management subcommands.
#[derive(Debug, Subcommand)]
enum CacheCommand {
    /// Scan paths and add font metadata to the cache
    Add(CacheAddArgs),
    /// List all cached font entries
    List(CacheListArgs),
    /// Query the cache without scanning the filesystem
    Find(Box<CacheFindArgs>),
    /// Remove entries for fonts that no longer exist on disk
    Clean(CacheCleanArgs),
    /// Show cache location, size, and entry count
    Info(CacheInfoArgs),
}

/// HTTP server configuration.
#[derive(Debug, Args)]
struct ServeArgs {
    /// Address to bind (host:port)
    #[arg(long = "bind", default_value = "127.0.0.1:8765")]
    bind: String,
}

/// Arguments for `cache add`.
#[derive(Debug, Args)]
struct CacheAddArgs {
    /// Paths to scan (directories or individual font files)
    #[arg(
        value_hint = ValueHint::DirPath,
        required_unless_present_any = ["system_fonts", "stdin_paths"]
    )]
    paths: Vec<PathBuf>,

    /// Read additional paths from stdin, one per line
    #[arg(long = "stdin-paths", action = ArgAction::SetTrue)]
    stdin_paths: bool,

    /// Include platform-default system font directories
    #[arg(long = "system-fonts", action = ArgAction::SetTrue)]
    system_fonts: bool,

    /// Follow symlinks during directory traversal
    #[arg(long = "follow-symlinks", action = ArgAction::SetTrue)]
    follow_symlinks: bool,

    /// Number of parallel worker threads
    #[arg(short = 'J', long = "jobs", value_hint = ValueHint::Other)]
    jobs: Option<usize>,

    /// Override cache file location
    #[arg(long = "cache-path", value_hint = ValueHint::FilePath)]
    cache_path: Option<PathBuf>,

    /// Use LMDB index backend instead of JSON cache
    #[arg(long = "index", action = ArgAction::SetTrue)]
    use_index: bool,

    /// Override LMDB index directory
    #[arg(long = "index-path", value_hint = ValueHint::DirPath)]
    index_path: Option<PathBuf>,
}

/// Output format options shared across subcommands.
#[derive(Debug, Args, Clone)]
struct OutputArgs {
    /// Output as a single JSON array
    #[arg(long = "json", action = ArgAction::SetTrue, conflicts_with = "ndjson")]
    json: bool,

    /// Output as newline-delimited JSON (one object per line)
    #[arg(long = "ndjson", action = ArgAction::SetTrue)]
    ndjson: bool,

    /// Output file paths only (with #index for TTC faces)
    #[arg(
        long = "paths",
        action = ArgAction::SetTrue,
        conflicts_with_all = ["json", "ndjson", "columns"]
    )]
    paths: bool,

    /// Output as aligned columns
    #[arg(long = "columns", action = ArgAction::SetTrue)]
    columns: bool,

    /// Show individual TTC/OTC collection faces (path#index); default deduplicates by path
    #[arg(long = "collections", action = ArgAction::SetTrue)]
    collections: bool,

    /// Colorize output (auto detects terminal)
    #[arg(long = "color", default_value_t = ColorChoice::Auto, value_enum)]
    color: ColorChoice,
}

#[derive(Debug, Args)]
struct CacheListArgs {
    /// Override cache location (defaults to ~/.cache/typg/cache.json)
    #[arg(long = "cache-path", value_hint = ValueHint::FilePath)]
    cache_path: Option<PathBuf>,

    /// Use high-performance LMDB index instead of JSON cache (requires hpindex feature)
    #[arg(long = "index", action = ArgAction::SetTrue)]
    use_index: bool,

    /// Override index directory (defaults to ~/.cache/typg/index/)
    #[arg(long = "index-path", value_hint = ValueHint::DirPath)]
    index_path: Option<PathBuf>,

    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct CacheFindArgs {
    /// Override cache location (defaults to ~/.cache/typg/cache.json)
    #[arg(long = "cache-path", value_hint = ValueHint::FilePath)]
    cache_path: Option<PathBuf>,

    /// Use high-performance LMDB index instead of JSON cache (requires hpindex feature)
    #[arg(long = "index", action = ArgAction::SetTrue)]
    use_index: bool,

    /// Override index directory (defaults to ~/.cache/typg/index/)
    #[arg(long = "index-path", value_hint = ValueHint::DirPath)]
    index_path: Option<PathBuf>,

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

    /// Regex patterns that must match creator info (copyright, trademark, manufacturer, designer, description, URLs, license)
    #[arg(short = 'c', long = "creator", value_hint = ValueHint::Other)]
    creator_patterns: Vec<String>,

    /// Regex patterns that must match license info (copyright, license description, license URL)
    #[arg(short = 'l', long = "license", value_hint = ValueHint::Other)]
    license_patterns: Vec<String>,

    /// Unicode codepoints or ranges (e.g. U+0041-U+0044,B)
    #[arg(short = 'u', long = "codepoints", value_delimiter = ',', value_hint = ValueHint::Other)]
    codepoints: Vec<String>,

    /// Require fonts to cover this text sample
    #[arg(short = 't', long = "text")]
    text: Option<String>,

    /// Only include variable fonts
    #[arg(short = 'v', long = "variable", action = ArgAction::SetTrue)]
    variable: bool,

    /// Match OS/2 weight class (single value like 400 or range like 300-500)
    #[arg(short = 'w', long = "weight", value_hint = ValueHint::Other)]
    weight: Option<String>,

    /// Match OS/2 width class (1-9, single value or range)
    #[arg(short = 'W', long = "width", value_hint = ValueHint::Other)]
    width: Option<String>,

    /// Match OS/2 family class (major like 8 or major.subclass like 8.11; accepts names like sans)
    #[arg(long = "family-class", value_hint = ValueHint::Other)]
    family_class: Option<String>,

    /// Only output the count of matching fonts (useful for scripting)
    #[arg(long = "count", action = ArgAction::SetTrue, conflicts_with_all = ["json", "ndjson", "paths", "columns"])]
    count_only: bool,

    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct CacheCleanArgs {
    /// Override cache location (defaults to ~/.cache/typg/cache.json)
    #[arg(long = "cache-path", value_hint = ValueHint::FilePath)]
    cache_path: Option<PathBuf>,

    /// Use high-performance LMDB index instead of JSON cache (requires hpindex feature)
    #[arg(long = "index", action = ArgAction::SetTrue)]
    use_index: bool,

    /// Override index directory (defaults to ~/.cache/typg/index/)
    #[arg(long = "index-path", value_hint = ValueHint::DirPath)]
    index_path: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct CacheInfoArgs {
    /// Override cache location (defaults to ~/.cache/typg/cache.json)
    #[arg(long = "cache-path", value_hint = ValueHint::FilePath)]
    cache_path: Option<PathBuf>,

    /// Use high-performance LMDB index instead of JSON cache (requires hpindex feature)
    #[arg(long = "index", action = ArgAction::SetTrue)]
    use_index: bool,

    /// Override index directory (defaults to ~/.cache/typg/index/)
    #[arg(long = "index-path", value_hint = ValueHint::DirPath)]
    index_path: Option<PathBuf>,

    /// Output as JSON
    #[arg(long = "json", action = ArgAction::SetTrue)]
    json: bool,
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

    /// Regex patterns that must match creator info (copyright, trademark, manufacturer, designer, description, URLs, license)
    #[arg(short = 'c', long = "creator", value_hint = ValueHint::Other)]
    creator_patterns: Vec<String>,

    /// Regex patterns that must match license info (copyright, license description, license URL)
    #[arg(short = 'l', long = "license", value_hint = ValueHint::Other)]
    license_patterns: Vec<String>,

    /// Unicode codepoints or ranges (e.g. U+0041-U+0044,B)
    #[arg(short = 'u', long = "codepoints", value_delimiter = ',', value_hint = ValueHint::Other)]
    codepoints: Vec<String>,

    /// Require fonts to cover this text sample
    #[arg(short = 't', long = "text")]
    text: Option<String>,

    /// Only include variable fonts
    #[arg(short = 'v', long = "variable", action = ArgAction::SetTrue)]
    variable: bool,

    /// Match OS/2 weight class (single value like 400 or range like 300-500)
    #[arg(short = 'w', long = "weight", value_hint = ValueHint::Other)]
    weight: Option<String>,

    /// Match OS/2 width class (1-9, single value or range)
    #[arg(short = 'W', long = "width", value_hint = ValueHint::Other)]
    width: Option<String>,

    /// Match OS/2 family class (major like 8 or major.subclass like 8.11; accepts names like sans)
    #[arg(long = "family-class", value_hint = ValueHint::Other)]
    family_class: Option<String>,

    /// Follow symlinks while walking paths
    #[arg(long = "follow-symlinks", action = ArgAction::SetTrue)]
    follow_symlinks: bool,

    /// Number of worker threads (defaults to CPU count)
    #[arg(short = 'J', long = "jobs", value_hint = ValueHint::Other)]
    jobs: Option<usize>,

    /// Emit a single JSON array
    #[arg(long = "json", action = ArgAction::SetTrue, conflicts_with = "ndjson")]
    json: bool,

    /// Emit newline-delimited JSON
    #[arg(long = "ndjson", action = ArgAction::SetTrue)]
    ndjson: bool,

    /// Emit newline-delimited font paths (with #index for TTC)
    #[arg(
        long = "paths",
        action = ArgAction::SetTrue,
        conflicts_with_all = ["json", "ndjson", "columns"]
    )]
    paths_only: bool,

    /// Format output as padded columns
    #[arg(long = "columns", action = ArgAction::SetTrue)]
    columns: bool,

    /// Show individual TTC/OTC collection faces (path#index); default deduplicates by path
    #[arg(long = "collections", action = ArgAction::SetTrue)]
    collections: bool,

    /// Only output the count of matching fonts (useful for scripting)
    #[arg(long = "count", action = ArgAction::SetTrue, conflicts_with_all = ["json", "ndjson", "paths_only", "columns"])]
    count_only: bool,

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

/// Parse CLI arguments and dispatch to the appropriate handler.
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let quiet = cli.quiet;

    match cli.command {
        Command::Find(args) => run_find(*args),
        Command::Cache(cmd) => match cmd {
            CacheCommand::Add(args) => run_cache_add(args, quiet),
            CacheCommand::List(args) => run_cache_list(args),
            CacheCommand::Find(args) => run_cache_find(*args),
            CacheCommand::Clean(args) => run_cache_clean(args, quiet),
            CacheCommand::Info(args) => run_cache_info(args),
        },
        Command::Serve(args) => run_serve(args),
    }
}

/// Search directories for matching fonts, streaming results where possible.
fn run_find(args: FindArgs) -> Result<()> {
    if matches!(args.jobs, Some(0)) {
        return Err(anyhow!("--jobs must be at least 1"));
    }

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
        jobs: args.jobs,
    };

    let output = OutputFormat::from_find(&args);

    // Formats that need all results before writing
    if args.count_only || output.json || output.columns {
        let matches = search(&paths, &query, &opts)?;
        if args.count_only {
            println!("{}", matches.len());
            return Ok(());
        }
        return write_matches(&matches, &output);
    }

    // Stream results to stdout as they're found
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::scope(|s| -> Result<()> {
        let handle = s.spawn(|| search_streaming(&paths, &query, &opts, tx));

        let stdout = io::stdout();
        let mut w = stdout.lock();
        let use_color = match output.color {
            ColorChoice::Always => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => w.is_terminal(),
        };

        let mut seen = std::collections::HashSet::new();
        for m in rx {
            if output.paths {
                if output.collections {
                    let _ = writeln!(w, "{}", m.source.path_with_index());
                } else if seen.insert(m.source.path.clone()) {
                    let _ = writeln!(w, "{}", m.source.path.display());
                }
            } else if output.ndjson {
                if let Ok(line) = serde_json::to_string(&m) {
                    let _ = w.write_all(line.as_bytes());
                    let _ = w.write_all(b"\n");
                }
            } else if output.collections {
                let rendered = render_path(&m, use_color, true);
                let _ = writeln!(w, "{rendered}");
            } else if seen.insert(m.source.path.clone()) {
                let rendered = render_path(&m, use_color, false);
                let _ = writeln!(w, "{rendered}");
            }
        }

        match handle.join() {
            Ok(result) => result,
            Err(_) => Err(anyhow!("search thread panicked")),
        }
    })
}

fn run_serve(args: ServeArgs) -> Result<()> {
    let runtime = Builder::new_multi_thread().enable_all().build()?;
    runtime.block_on(server::serve(&args.bind))
}

#[derive(Clone, Debug)]
struct OutputFormat {
    json: bool,
    ndjson: bool,
    paths: bool,
    columns: bool,
    collections: bool,
    color: ColorChoice,
}

impl OutputFormat {
    fn from_find(args: &FindArgs) -> Self {
        Self {
            json: args.json,
            ndjson: args.ndjson,
            paths: args.paths_only,
            columns: args.columns,
            collections: args.collections,
            color: args.color,
        }
    }

    fn from_output(args: &OutputArgs) -> Self {
        Self {
            json: args.json,
            ndjson: args.ndjson,
            paths: args.paths,
            columns: args.columns,
            collections: args.collections,
            color: args.color,
        }
    }
}

fn write_matches(matches: &[TypgFontFaceMatch], format: &OutputFormat) -> Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let use_color = match format.color {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => handle.is_terminal(),
    };

    if format.paths {
        write_paths(matches, &mut handle, format.collections)?;
    } else if format.ndjson {
        write_ndjson(matches, &mut handle)?;
    } else if format.json {
        write_json_pretty(matches, &mut handle)?;
    } else if format.columns {
        write_columns(matches, &mut handle, use_color, format.collections)?;
    } else {
        write_plain(matches, &mut handle, use_color, format.collections)?;
    }

    Ok(())
}

/// Build a `Query` from the CLI filter arguments.
fn build_query(args: &FindArgs) -> Result<Query> {
    build_query_from_parts(
        &args.axes,
        &args.features,
        &args.scripts,
        &args.tables,
        &args.name_patterns,
        &args.creator_patterns,
        &args.license_patterns,
        &args.codepoints,
        &args.text,
        args.variable,
        &args.weight,
        &args.width,
        &args.family_class,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_query_from_parts(
    axes: &[String],
    features: &[String],
    scripts: &[String],
    tables: &[String],
    name_patterns: &[String],
    creator_patterns: &[String],
    license_patterns: &[String],
    codepoints: &[String],
    text: &Option<String>,
    variable: bool,
    weight: &Option<String>,
    width: &Option<String>,
    family_class: &Option<String>,
) -> Result<Query> {
    let axes = parse_tag_list(axes)?;
    let features = parse_tag_list(features)?;
    let scripts = parse_tag_list(scripts)?;
    let tables = parse_tag_list(tables)?;
    let name_patterns = compile_patterns(name_patterns)?;
    let creator_patterns = compile_patterns(creator_patterns)?;
    let license_patterns = compile_patterns(license_patterns)?;
    let mut codepoints = parse_codepoints(codepoints)?;
    let weight_range = parse_optional_range(weight)?;
    let width_range = parse_optional_range(width)?;
    let family_class = parse_optional_family_class(family_class)?;

    if let Some(text) = text {
        codepoints.extend(text.chars());
    }

    dedup_chars(&mut codepoints);

    Ok(Query::new()
        .with_axes(axes)
        .with_features(features)
        .with_scripts(scripts)
        .with_tables(tables)
        .with_name_patterns(name_patterns)
        .with_creator_patterns(creator_patterns)
        .with_license_patterns(license_patterns)
        .with_codepoints(codepoints)
        .require_variable(variable)
        .with_weight_range(weight_range)
        .with_width_range(width_range)
        .with_family_class(family_class))
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

fn parse_optional_range(raw: &Option<String>) -> Result<Option<RangeInclusive<u16>>> {
    match raw {
        Some(value) => Ok(Some(parse_u16_range(value)?)),
        None => Ok(None),
    }
}

fn parse_optional_family_class(raw: &Option<String>) -> Result<Option<FamilyClassFilter>> {
    match raw {
        Some(value) => Ok(Some(parse_family_class(value)?)),
        None => Ok(None),
    }
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

fn write_plain(
    matches: &[TypgFontFaceMatch],
    mut w: impl Write,
    color: bool,
    collections: bool,
) -> Result<()> {
    if collections {
        for item in matches {
            let rendered = render_path(item, color, true);
            writeln!(w, "{rendered}")?;
        }
    } else {
        let mut seen = std::collections::HashSet::new();
        for item in matches {
            if seen.insert(item.source.path.clone()) {
                let rendered = render_path(item, color, false);
                writeln!(w, "{rendered}")?;
            }
        }
    }
    Ok(())
}

fn write_paths(
    matches: &[TypgFontFaceMatch],
    mut w: impl Write,
    collections: bool,
) -> Result<()> {
    if collections {
        for item in matches {
            writeln!(w, "{}", item.source.path_with_index())?;
        }
    } else {
        let mut seen = std::collections::HashSet::new();
        for item in matches {
            if seen.insert(item.source.path.clone()) {
                writeln!(w, "{}", item.source.path.display())?;
            }
        }
    }
    Ok(())
}

fn write_columns(
    matches: &[TypgFontFaceMatch],
    mut w: impl Write,
    color: bool,
    collections: bool,
) -> Result<()> {
    let mut rows: Vec<(String, String, String)> = matches
        .iter()
        .map(|m| {
            let path = if collections {
                m.source.path_with_index()
            } else {
                m.source.path.display().to_string()
            };
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

fn render_path(item: &TypgFontFaceMatch, color: bool, collections: bool) -> String {
    let rendered = if collections {
        item.source.path_with_index()
    } else {
        item.source.path.display().to_string()
    };
    apply_color(&rendered, color, AnsiColor::Cyan)
}

fn run_cache_add(args: CacheAddArgs, quiet: bool) -> Result<()> {
    if matches!(args.jobs, Some(0)) {
        return Err(anyhow!("--jobs must be at least 1"));
    }

    #[cfg(feature = "hpindex")]
    if args.use_index {
        return run_cache_add_index(args, quiet);
    }

    #[cfg(not(feature = "hpindex"))]
    if args.use_index {
        return Err(anyhow!(
            "--index requires the hpindex feature; rebuild with: cargo build --features hpindex"
        ));
    }

    let stdin = io::stdin();
    let paths = gather_paths(
        &args.paths,
        args.stdin_paths,
        args.system_fonts,
        stdin.lock(),
    )?;

    let opts = SearchOptions {
        follow_symlinks: args.follow_symlinks,
        jobs: args.jobs,
    };
    let additions = search(&paths, &Query::new(), &opts)?;

    let cache_path = resolve_cache_path(&args.cache_path)?;
    let existing = if cache_path.exists() {
        load_cache(&cache_path)?
    } else {
        Vec::new()
    };

    let merged = merge_entries(existing, additions);
    write_cache(&cache_path, &merged)?;

    if !quiet {
        eprintln!(
            "cached {} font faces at {}",
            merged.len(),
            cache_path.display()
        );
    }
    Ok(())
}

fn run_cache_list(args: CacheListArgs) -> Result<()> {
    #[cfg(feature = "hpindex")]
    if args.use_index {
        return run_cache_list_index(args);
    }

    #[cfg(not(feature = "hpindex"))]
    if args.use_index {
        return Err(anyhow!(
            "--index requires the hpindex feature; rebuild with: cargo build --features hpindex"
        ));
    }

    let cache_path = resolve_cache_path(&args.cache_path)?;
    let entries = load_cache(&cache_path)?;
    let output = OutputFormat::from_output(&args.output);
    write_matches(&entries, &output)
}

fn run_cache_find(args: CacheFindArgs) -> Result<()> {
    #[cfg(feature = "hpindex")]
    if args.use_index {
        return run_cache_find_index(args);
    }

    #[cfg(not(feature = "hpindex"))]
    if args.use_index {
        return Err(anyhow!(
            "--index requires the hpindex feature; rebuild with: cargo build --features hpindex"
        ));
    }

    let cache_path = resolve_cache_path(&args.cache_path)?;
    let entries = load_cache(&cache_path)?;
    let query = build_query_from_parts(
        &args.axes,
        &args.features,
        &args.scripts,
        &args.tables,
        &args.name_patterns,
        &args.creator_patterns,
        &args.license_patterns,
        &args.codepoints,
        &args.text,
        args.variable,
        &args.weight,
        &args.width,
        &args.family_class,
    )?;

    let matches = filter_cached(&entries, &query);

    if args.count_only {
        println!("{}", matches.len());
        return Ok(());
    }

    let output = OutputFormat::from_output(&args.output);
    write_matches(&matches, &output)
}

fn run_cache_clean(args: CacheCleanArgs, quiet: bool) -> Result<()> {
    #[cfg(feature = "hpindex")]
    if args.use_index {
        return run_cache_clean_index(args, quiet);
    }

    #[cfg(not(feature = "hpindex"))]
    if args.use_index {
        return Err(anyhow!(
            "--index requires the hpindex feature; rebuild with: cargo build --features hpindex"
        ));
    }

    let cache_path = resolve_cache_path(&args.cache_path)?;
    let entries = load_cache(&cache_path)?;
    let before = entries.len();
    let pruned = prune_missing(entries);
    let after = pruned.len();

    write_cache(&cache_path, &pruned)?;
    if !quiet {
        eprintln!(
            "removed {} missing entries ({} → {})",
            before.saturating_sub(after),
            before,
            after
        );
    }
    Ok(())
}

fn run_cache_info(args: CacheInfoArgs) -> Result<()> {
    #[cfg(feature = "hpindex")]
    if args.use_index {
        return run_cache_info_index(args);
    }

    #[cfg(not(feature = "hpindex"))]
    if args.use_index {
        return Err(anyhow!(
            "--index requires the hpindex feature; rebuild with: cargo build --features hpindex"
        ));
    }

    let cache_path = resolve_cache_path(&args.cache_path)?;

    if !cache_path.exists() {
        if args.json {
            println!(r#"{{"exists":false,"path":"{}"}}"#, cache_path.display());
        } else {
            println!("Cache does not exist at {}", cache_path.display());
        }
        return Ok(());
    }

    let entries = load_cache(&cache_path)?;
    let file_meta = fs::metadata(&cache_path)?;
    let size_bytes = file_meta.len();

    if args.json {
        let info = serde_json::json!({
            "exists": true,
            "path": cache_path.display().to_string(),
            "type": "json",
            "entries": entries.len(),
            "size_bytes": size_bytes,
        });
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!("Cache: {}", cache_path.display());
        println!("Type:  JSON");
        println!("Fonts: {}", entries.len());
        println!("Size:  {} bytes", size_bytes);
    }

    Ok(())
}

fn resolve_cache_path(custom: &Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = custom {
        return Ok(path.clone());
    }

    if let Ok(env_override) = env::var("TYPOG_CACHE_PATH") {
        return Ok(PathBuf::from(env_override));
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(local_appdata) = env::var_os("LOCALAPPDATA") {
            return Ok(PathBuf::from(local_appdata).join("typg").join("cache.json"));
        }
        if let Some(home) = env::var_os("HOME") {
            return Ok(PathBuf::from(home).join("AppData/Local/typg/cache.json"));
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(xdg) = env::var_os("XDG_CACHE_HOME") {
            return Ok(PathBuf::from(xdg).join("typg").join("cache.json"));
        }
        if let Some(home) = env::var_os("HOME") {
            return Ok(PathBuf::from(home)
                .join(".cache")
                .join("typg")
                .join("cache.json"));
        }
    }

    Err(anyhow!(
        "--cache-path is required because no cache directory could be detected"
    ))
}

/// Resolve the index directory path.
#[cfg_attr(not(feature = "hpindex"), allow(dead_code))]
fn resolve_index_path(custom: &Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = custom {
        return Ok(path.clone());
    }

    if let Ok(env_override) = env::var("TYPOG_INDEX_PATH") {
        return Ok(PathBuf::from(env_override));
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(local_appdata) = env::var_os("LOCALAPPDATA") {
            return Ok(PathBuf::from(local_appdata).join("typg").join("index"));
        }
        if let Some(home) = env::var_os("HOME") {
            return Ok(PathBuf::from(home).join("AppData/Local/typg/index"));
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(xdg) = env::var_os("XDG_CACHE_HOME") {
            return Ok(PathBuf::from(xdg).join("typg").join("index"));
        }
        if let Some(home) = env::var_os("HOME") {
            return Ok(PathBuf::from(home)
                .join(".cache")
                .join("typg")
                .join("index"));
        }
    }

    Err(anyhow!(
        "--index-path is required because no cache directory could be detected"
    ))
}

/// Load cached font entries from disk. Tries JSON array first, falls back to NDJSON.
fn load_cache(path: &Path) -> Result<Vec<TypgFontFaceMatch>> {
    let file = File::open(path).with_context(|| format!("opening cache {}", path.display()))?;
    let reader = BufReader::new(file);

    match serde_json::from_reader(reader) {
        Ok(entries) => Ok(entries),
        Err(_) => {
            // Fall back to NDJSON parsing for forward compatibility
            let file =
                File::open(path).with_context(|| format!("re-opening cache {}", path.display()))?;
            let reader = BufReader::new(file);
            let stream = Deserializer::from_reader(reader).into_iter::<TypgFontFaceMatch>();
            let mut entries = Vec::new();
            for item in stream {
                entries.push(item?);
            }
            Ok(entries)
        }
    }
}

/// Write font entries to the cache file as pretty-printed JSON.
fn write_cache(path: &Path, entries: &[TypgFontFaceMatch]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }

    let file = File::create(path).with_context(|| format!("creating cache {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, entries)
        .with_context(|| format!("writing cache {}", path.display()))?;
    writer.flush()?;
    Ok(())
}

fn merge_entries(
    existing: Vec<TypgFontFaceMatch>,
    additions: Vec<TypgFontFaceMatch>,
) -> Vec<TypgFontFaceMatch> {
    let mut map: HashMap<(PathBuf, Option<u32>), TypgFontFaceMatch> = HashMap::new();

    for entry in existing.into_iter().chain(additions.into_iter()) {
        map.insert(cache_key(&entry), entry);
    }

    let mut merged: Vec<TypgFontFaceMatch> = map.into_values().collect();
    sort_entries(&mut merged);
    merged
}

fn prune_missing(entries: Vec<TypgFontFaceMatch>) -> Vec<TypgFontFaceMatch> {
    let mut pruned: Vec<TypgFontFaceMatch> = entries
        .into_iter()
        .filter(|entry| entry.source.path.exists())
        .collect();
    sort_entries(&mut pruned);
    pruned
}

fn sort_entries(entries: &mut [TypgFontFaceMatch]) {
    entries.sort_by(|a, b| {
        a.source
            .path
            .cmp(&b.source.path)
            .then_with(|| a.source.ttc_index.cmp(&b.source.ttc_index))
    });
}

fn cache_key(entry: &TypgFontFaceMatch) -> (PathBuf, Option<u32>) {
    (entry.source.path.clone(), entry.source.ttc_index)
}

// ============================================================================
// High-performance index implementations (LMDB + Roaring Bitmaps)
// ============================================================================

#[cfg(feature = "hpindex")]
fn run_cache_add_index(args: CacheAddArgs, quiet: bool) -> Result<()> {
    use std::time::SystemTime;

    let stdin = io::stdin();
    let paths = gather_paths(
        &args.paths,
        args.stdin_paths,
        args.system_fonts,
        stdin.lock(),
    )?;

    let index_path = resolve_index_path(&args.index_path)?;
    let index = FontIndex::open(&index_path)?;

    // Use the existing search pipeline to discover and extract metadata.
    let opts = SearchOptions {
        follow_symlinks: args.follow_symlinks,
        jobs: args.jobs,
    };
    let additions = search(&paths, &Query::new(), &opts)?;

    // Write to index in a single transaction.
    let mut writer = index.writer()?;
    let mut added = 0usize;
    let mut skipped = 0usize;

    for entry in additions {
        // Get file mtime for incremental update detection.
        let mtime = entry
            .source
            .path
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        // Check if update is needed.
        if !writer.needs_update(&entry.source.path, mtime)? {
            skipped += 1;
            continue;
        }

        writer.add_font(
            &entry.source.path,
            entry.source.ttc_index,
            mtime,
            entry.metadata.names.clone(),
            &entry.metadata.axis_tags,
            &entry.metadata.feature_tags,
            &entry.metadata.script_tags,
            &entry.metadata.table_tags,
            &entry.metadata.codepoints,
            entry.metadata.is_variable,
            entry.metadata.weight_class,
            entry.metadata.width_class,
            entry.metadata.family_class,
        )?;
        added += 1;
    }

    writer.commit()?;

    if !quiet {
        let total = index.count()?;
        eprintln!(
            "indexed {} font faces at {} (added: {}, skipped: {})",
            total,
            index_path.display(),
            added,
            skipped
        );
    }

    Ok(())
}

#[cfg(feature = "hpindex")]
fn run_cache_list_index(args: CacheListArgs) -> Result<()> {
    let index_path = resolve_index_path(&args.index_path)?;
    let index = FontIndex::open(&index_path)?;
    let reader = index.reader()?;
    let entries = reader.list_all()?;
    let output = OutputFormat::from_output(&args.output);
    write_matches(&entries, &output)
}

#[cfg(feature = "hpindex")]
fn run_cache_find_index(args: CacheFindArgs) -> Result<()> {
    let index_path = resolve_index_path(&args.index_path)?;
    let index = FontIndex::open(&index_path)?;

    let query = build_query_from_parts(
        &args.axes,
        &args.features,
        &args.scripts,
        &args.tables,
        &args.name_patterns,
        &args.creator_patterns,
        &args.license_patterns,
        &args.codepoints,
        &args.text,
        args.variable,
        &args.weight,
        &args.width,
        &args.family_class,
    )?;

    let reader = index.reader()?;
    let matches = reader.find(&query)?;

    if args.count_only {
        println!("{}", matches.len());
        return Ok(());
    }

    let output = OutputFormat::from_output(&args.output);
    write_matches(&matches, &output)
}

#[cfg(feature = "hpindex")]
fn run_cache_clean_index(args: CacheCleanArgs, quiet: bool) -> Result<()> {
    let index_path = resolve_index_path(&args.index_path)?;
    let index = FontIndex::open(&index_path)?;

    let mut writer = index.writer()?;
    let (before, after) = writer.prune_missing()?;
    writer.commit()?;

    if !quiet {
        eprintln!(
            "removed {} missing entries ({} → {})",
            before.saturating_sub(after),
            before,
            after
        );
    }
    Ok(())
}

#[cfg(feature = "hpindex")]
fn run_cache_info_index(args: CacheInfoArgs) -> Result<()> {
    let index_path = resolve_index_path(&args.index_path)?;

    if !index_path.exists() {
        if args.json {
            println!(r#"{{"exists":false,"path":"{}"}}"#, index_path.display());
        } else {
            println!("Index does not exist at {}", index_path.display());
        }
        return Ok(());
    }

    let index = FontIndex::open(&index_path)?;
    let count = index.count()?;

    // Calculate total directory size (non-recursive, LMDB is flat).
    let size_bytes: u64 = fs::read_dir(&index_path)?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum();

    if args.json {
        let info = serde_json::json!({
            "exists": true,
            "path": index_path.display().to_string(),
            "type": "lmdb",
            "entries": count,
            "size_bytes": size_bytes,
        });
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!("Index: {}", index_path.display());
        println!("Type:  LMDB");
        println!("Fonts: {}", count);
        println!("Size:  {} bytes", size_bytes);
    }

    Ok(())
}

#[cfg(test)]
mod tests;
