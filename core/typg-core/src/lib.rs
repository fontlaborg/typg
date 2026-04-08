/// typg-core: the engine behind fast font search.
///
/// Point it at a folder of fonts. Tell it what you need — Arabic script support,
/// a weight axis, ligatures, a specific glyph. It reads every font file in
/// parallel, extracts the OpenType metadata that matters, and hands back the
/// matches. Thousands of fonts in under a second on a modern machine.
///
/// # How a search works, end to end
///
/// 1. **Discovery** ([`discovery`]) walks your directories, collecting every
///    `.ttf`, `.otf`, `.ttc`, and `.otc` file it finds. Broken symlinks and
///    permission errors get a warning, not a crash.
///
/// 2. **Search** ([`search`]) opens each file with the `read-fonts` and `skrifa`
///    crates (Google's Rust font-parsing libraries), pulls out metadata —
///    names, axes, features, scripts, tables, codepoints, weight/width
///    classification — and checks it against your query. This step runs on
///    all available CPU cores via `rayon`.
///
/// 3. **Query** ([`query`]) is the filter specification. Every criterion is
///    optional; an empty query matches everything. Criteria combine with AND
///    logic: a font must satisfy *all* of them to appear in results.
///
/// 4. **Output** ([`output`]) serializes results to JSON or NDJSON for
///    downstream tools and pipelines.
///
/// 5. **Tags** ([`tags`]) handles parsing and formatting of OpenType tags —
///    the four-character codes (`wght`, `liga`, `latn`, `GSUB`) that identify
///    axes, features, scripts, and tables inside a font.
///
/// 6. **Index** ([`index`], behind the `hpindex` feature flag) stores extracted
///    metadata in an LMDB database with Roaring Bitmap inverted indices.
///    Queries that would take seconds over thousands of files on disk take
///    milliseconds against the index.
///
/// # Quick example
///
/// Find all variable fonts with Arabic script support and a weight axis:
///
/// ```rust,no_run
/// use std::path::PathBuf;
/// use typg_core::query::Query;
/// use typg_core::search::{search, SearchOptions};
/// use typg_core::tags::tag4;
///
/// let query = Query::new()
///     .with_scripts(vec![tag4("arab").unwrap()])
///     .with_axes(vec![tag4("wght").unwrap()])
///     .require_variable(true);
///
/// let dirs = vec![PathBuf::from("/Library/Fonts")];
/// let results = search(&dirs, &query, &SearchOptions::default())?;
///
/// for font in &results {
///     println!("{}", font.source.path_with_index());
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Font vocabulary cheat sheet
///
/// | Term | What it means |
/// |------|--------------|
/// | **OpenType** | The modern font format standard. A `.ttf` or `.otf` file is an OpenType font. |
/// | **Variable font** | A single font file that contains a continuous range of weights, widths, or other design variations. Controlled by *axes*. |
/// | **Axis** | A dimension of variation in a variable font. `wght` = weight (thin→black), `wdth` = width (condensed→expanded), `opsz` = optical size. |
/// | **Feature** | An OpenType layout feature like `liga` (ligatures), `smcp` (small caps), or `kern` (kerning). Controls how glyphs are substituted or positioned. |
/// | **Script** | A writing system tag like `latn` (Latin), `arab` (Arabic), `cyrl` (Cyrillic). Tells the shaping engine which rules to apply. |
/// | **Table** | A named data block inside the font file. `GSUB` holds glyph substitution rules, `GPOS` holds positioning rules, `OS/2` holds classification metadata. |
/// | **TTC/OTC** | TrueType/OpenType Collection — a single file bundling multiple font faces. Each face has a numeric index. |
/// | **cmap** | The character map table. Maps Unicode codepoints to glyph IDs — it's how the font says "I can draw this character." |
/// | **OS/2** | A metadata table carrying weight class, width class, font family classification, and other attributes originally designed for IBM's OS/2 operating system (the name stuck). |
///
/// Made by FontLab <https://www.fontlab.com/>
pub mod discovery;
#[cfg(feature = "hpindex")]
pub mod index;
pub mod output;
pub mod query;
pub mod search;
pub mod tags;
