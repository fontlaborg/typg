/// typg-core: font search engine.
///
/// Discovers font files on the filesystem, extracts OpenType metadata,
/// and evaluates queries against that metadata. Parallel via rayon.
///
/// ## Modules
///
/// - [`discovery`]: Filesystem traversal to find font files
/// - [`search`]: Metadata extraction and query evaluation
/// - [`query`]: Query construction and filter logic
/// - [`output`]: Result formatting (JSON, NDJSON)
/// - [`tags`]: OpenType tag parsing utilities
/// - [`index`]: LMDB-backed index (behind `hpindex` feature)
///
/// ## Example
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
/// Made by FontLab https://www.fontlab.com/
pub mod discovery;
#[cfg(feature = "hpindex")]
pub mod index;
pub mod output;
pub mod query;
pub mod search;
pub mod tags;
