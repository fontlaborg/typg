//! Python bindings for typg-core - where font discovery becomes genuinely pleasant.
//!
//! Finding the right font shouldn't feel like solving a mystery. These PyO3 bindings
//! create a smooth conversation between Python and Rust, letting you ask the typg-core
//! engine for exactly the fonts you need. Think of it as having a remarkably organized
//! friend who knows where every font lives - no frantic searching required.
//!
//! Built by FontLab (https://www.fontlab.com/) - people who understand fonts are just
//! characters with remarkable personalities.

use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use regex::Regex;
use typg_core::query::{
    parse_codepoint_list, parse_family_class, parse_tag_list, parse_u16_range, FamilyClassFilter,
    Query,
};
use typg_core::search::{
    filter_cached, search, SearchOptions, TypgFontFaceMatch, TypgFontFaceMeta, TypgFontSource,
};
use typg_core::tags::tag_to_string;

#[cfg(feature = "hpindex")]
use typg_core::index::FontIndex;

/// A font's comprehensive profile for thoughtful matchmaking.
///
/// Contains every essential detail needed for font filtering operations. Like a
/// well-crafted dating profile that answers all the important questions upfront -
/// location, capabilities, special talents, and whether this font enjoys changing
/// its appearance on demand.
#[derive(Clone, Debug, FromPyObject)]
struct MetadataInput {
    /// Absolute path where this font resides on disk
    path: PathBuf,
    /// All known names for this font family
    #[pyo3(default)]
    names: Vec<String>,
    /// Variable font variation axes supported
    #[pyo3(default)]
    axis_tags: Vec<String>,
    /// OpenType feature tags available for advanced typography
    #[pyo3(default)]
    feature_tags: Vec<String>,
    /// Script tags indicating supported writing systems
    #[pyo3(default)]
    script_tags: Vec<String>,
    /// Font table tags included in this font file
    #[pyo3(default)]
    table_tags: Vec<String>,
    /// Unicode characters this font can render
    #[pyo3(default)]
    codepoints: Vec<String>,
    /// Indicates whether this is a variable font
    #[pyo3(default)]
    is_variable: bool,
    /// TTC collection index if font is part of a collection
    #[pyo3(default)]
    ttc_index: Option<u32>,
    /// Font weight class (100-900, where 400 is regular)
    #[pyo3(default)]
    weight_class: Option<u16>,
    /// Font width class (1-9, where 5 is normal)
    #[pyo3(default)]
    width_class: Option<u16>,
    /// Font family class classification bits
    #[pyo3(default)]
    family_class: Option<u16>,
}

/// Search directories for fonts matching your specifications.
///
/// Scans the provided filesystem paths to locate fonts that meet your criteria.
/// This function handles the heavy lifting of directory traversal and font parsing,
/// returning comprehensive metadata for each match. Performance scales with the
/// number of worker threads specified.
#[pyfunction]
#[pyo3(
    signature = (
        paths,
        axes=None,
        features=None,
        scripts=None,
        tables=None,
        names=None,
        codepoints=None,
        text=None,
        weight=None,
        width=None,
        family_class=None,
        variable=false,
        follow_symlinks=false,
        jobs=None
    )
)]
#[allow(clippy::too_many_arguments)]
fn find_py(
    py: Python<'_>,
    paths: Vec<PathBuf>,
    axes: Option<Vec<String>>,
    features: Option<Vec<String>>,
    scripts: Option<Vec<String>>,
    tables: Option<Vec<String>>,
    names: Option<Vec<String>>,
    codepoints: Option<Vec<String>>,
    text: Option<String>,
    weight: Option<String>,
    width: Option<String>,
    family_class: Option<String>,
    variable: bool,
    follow_symlinks: bool,
    jobs: Option<usize>,
) -> PyResult<Vec<Py<PyAny>>> {
    // Can't search the void - need at least one place to look
    if paths.is_empty() {
        return Err(PyValueError::new_err(
            "at least one search path is required",
        ));
    }

    // Zero workers means nobody gets the job done
    if matches!(jobs, Some(0)) {
        return Err(PyValueError::new_err(
            "jobs must be at least 1 when provided",
        ));
    }

    // Build our font detective's search warrant
    let query = build_query(
        axes,
        features,
        scripts,
        tables,
        names,
        codepoints,
        text,
        weight,
        width,
        family_class,
        variable,
    )
    .map_err(to_py_err)?;

    // Configure the search team
    let opts = SearchOptions {
        follow_symlinks,
        jobs,
    };

    // Send out the search party and bring back the suspects
    let matches = search(&paths, &query, &opts).map_err(to_py_err)?;
    to_py_matches(py, matches)
}

/// Search directories and return only font file paths.
///
/// Optimized version of find_py that returns just the file paths instead of
/// full metadata. This reduces memory usage and processing overhead when you
/// only need locations, such as when building font catalogs or performing
/// batch operations.
#[pyfunction]
#[pyo3(
    signature = (
        paths,
        axes=None,
        features=None,
        scripts=None,
        tables=None,
        names=None,
        codepoints=None,
        text=None,
        weight=None,
        width=None,
        family_class=None,
        variable=false,
        follow_symlinks=false,
        jobs=None
    )
)]
#[allow(clippy::too_many_arguments)]
fn find_paths_py(
    paths: Vec<PathBuf>,
    axes: Option<Vec<String>>,
    features: Option<Vec<String>>,
    scripts: Option<Vec<String>>,
    tables: Option<Vec<String>>,
    names: Option<Vec<String>>,
    codepoints: Option<Vec<String>>,
    text: Option<String>,
    weight: Option<String>,
    width: Option<String>,
    family_class: Option<String>,
    variable: bool,
    follow_symlinks: bool,
    jobs: Option<usize>,
) -> PyResult<Vec<String>> {
    // Same validation as its fancier cousin - no empty searches
    if paths.is_empty() {
        return Err(PyValueError::new_err(
            "at least one search path is required",
        ));
    }

    // We need at least one worker bee in the hive
    if matches!(jobs, Some(0)) {
        return Err(PyValueError::new_err(
            "jobs must be at least 1 when provided",
        ));
    }

    // Build the same fancy query, but we'll only use the addresses
    let query = build_query(
        axes,
        features,
        scripts,
        tables,
        names,
        codepoints,
        text,
        weight,
        width,
        family_class,
        variable,
    )
    .map_err(to_py_err)?;

    // Search with the usual suspects
    let opts = SearchOptions {
        follow_symlinks,
        jobs,
    };
    let matches = search(&paths, &query, &opts).map_err(to_py_err)?;

    // Strip down to just the raw file paths - no frills attached
    Ok(matches
        .into_iter()
        .map(|m| m.source.path_with_index())
        .collect())
}

/// Filter pre-collected font metadata without filesystem access.
///
/// Operates entirely in memory on cached font metadata, avoiding expensive
/// filesystem operations. Ideal for font managers or applications that maintain
/// their own font databases and need fast filtering capabilities without disk I/O.
#[pyfunction]
#[pyo3(
    signature = (
        entries,
        axes=None,
        features=None,
        scripts=None,
        tables=None,
        names=None,
        codepoints=None,
        text=None,
        weight=None,
        width=None,
        family_class=None,
        variable=false
    )
)]
#[allow(clippy::too_many_arguments)]
fn filter_cached_py(
    py: Python<'_>,
    entries: Vec<MetadataInput>,
    axes: Option<Vec<String>>,
    features: Option<Vec<String>>,
    scripts: Option<Vec<String>>,
    tables: Option<Vec<String>>,
    names: Option<Vec<String>>,
    codepoints: Option<Vec<String>>,
    text: Option<String>,
    weight: Option<String>,
    width: Option<String>,
    family_class: Option<String>,
    variable: bool,
) -> PyResult<Vec<Py<PyAny>>> {
    // Convert the Python-friendly format to our internal type system
    let metadata = convert_metadata(entries).map_err(to_py_err)?;

    // Build the filter criteria - like a VIP list for fonts
    let query = build_query(
        axes,
        features,
        scripts,
        tables,
        names,
        codepoints,
        text,
        weight,
        width,
        family_class,
        variable,
    )
    .map_err(to_py_err)?;

    // Let the bouncer do the filtering - no filesystem required
    let matches = filter_cached(&metadata, &query);
    to_py_matches(py, matches)
}

/// Search fonts using a high-performance indexed database.
///
/// Leverages LMDB-based indexing for millisecond query performance across
/// thousands of fonts. The indexed database enables complex searches with
/// minimal overhead, making it ideal for applications requiring frequent
/// font queries.
///
/// Requires compilation with the hpindex feature flag enabled.
#[cfg(feature = "hpindex")]
#[pyfunction]
#[pyo3(
    signature = (
        index_path,
        axes=None,
        features=None,
        scripts=None,
        tables=None,
        names=None,
        codepoints=None,
        text=None,
        weight=None,
        width=None,
        family_class=None,
        variable=false
    )
)]
#[allow(clippy::too_many_arguments)]
fn find_indexed_py(
    py: Python<'_>,
    index_path: PathBuf,
    axes: Option<Vec<String>>,
    features: Option<Vec<String>>,
    scripts: Option<Vec<String>>,
    tables: Option<Vec<String>>,
    names: Option<Vec<String>>,
    codepoints: Option<Vec<String>>,
    text: Option<String>,
    weight: Option<String>,
    width: Option<String>,
    family_class: Option<String>,
    variable: bool,
) -> PyResult<Vec<Py<PyAny>>> {
    // Build our search criteria for the indexed database
    let query = build_query(
        axes,
        features,
        scripts,
        tables,
        names,
        codepoints,
        text,
        weight,
        width,
        family_class,
        variable,
    )
    .map_err(to_py_err)?;

    // Fire up the high-performance index and grab a read-only ticket
    let index = FontIndex::open(&index_path).map_err(to_py_err)?;
    let reader = index.reader().map_err(to_py_err)?;

    // Let the turbocharged database do its magic
    let matches = reader.find(&query).map_err(to_py_err)?;
    to_py_matches(py, matches)
}

/// List all fonts currently indexed in the database.
///
/// Returns metadata for every font stored in the indexed database without
/// applying any filters. Useful for inventory management, catalog generation,
/// or when you need a complete overview of available fonts.
///
/// Requires compilation with the hpindex feature flag enabled.
#[cfg(feature = "hpindex")]
#[pyfunction]
fn list_indexed_py(py: Python<'_>, index_path: PathBuf) -> PyResult<Vec<Py<PyAny>>> {
    // Open the database and get our VIP pass
    let index = FontIndex::open(&index_path).map_err(to_py_err)?;
    let reader = index.reader().map_err(to_py_err)?;

    // Roll out the red carpet for every font in the house
    let matches = reader.list_all().map_err(to_py_err)?;
    to_py_matches(py, matches)
}

/// Return the total number of fonts in the indexed database.
///
/// Provides a fast count of all fonts stored in the database without loading
/// metadata. Useful for progress indicators, statistics display, or quick
/// database size verification.
///
/// Requires compilation with the hpindex feature flag enabled.
#[cfg(feature = "hpindex")]
#[pyfunction]
fn count_indexed_py(index_path: PathBuf) -> PyResult<usize> {
    // Open up the database and ask for the head count
    let index = FontIndex::open(&index_path).map_err(to_py_err)?;
    index.count().map_err(to_py_err)
}

/// Convert Python metadata format to internal Rust structures.
///
/// Transforms Python-friendly input types into the strongly-typed structures
/// used internally by the search engine. This conversion layer handles type
/// safety and data normalization between the Python interface and Rust core.
fn convert_metadata(entries: Vec<MetadataInput>) -> Result<Vec<TypgFontFaceMatch>> {
    entries
        .into_iter()
        .map(|entry| {
            // Make sure we always have at least one name for this font
            let mut names = entry.names;
            if names.is_empty() {
                names.push(default_name(&entry.path));
            }

            // Build the complete font profile in our internal format
            Ok(TypgFontFaceMatch {
                source: TypgFontSource {
                    path: entry.path,
                    ttc_index: entry.ttc_index,
                },
                metadata: TypgFontFaceMeta {
                    names,
                    axis_tags: parse_tag_list(&entry.axis_tags)?,
                    feature_tags: parse_tag_list(&entry.feature_tags)?,
                    script_tags: parse_tag_list(&entry.script_tags)?,
                    table_tags: parse_tag_list(&entry.table_tags)?,
                    codepoints: parse_codepoints(&entry.codepoints)?,
                    is_variable: entry.is_variable,
                    weight_class: entry.weight_class,
                    width_class: entry.width_class,
                    family_class: entry
                        .family_class
                        .map(|raw| (((raw >> 8) & 0xFF) as u8, (raw & 0x00FF) as u8)),
                },
            })
        })
        .collect()
}

/// Generate a fallback name from the font file path.
///
/// Extracts a display name from the filename when font metadata doesn't
/// include names. Uses the file stem (filename without extension) as the
/// primary source, falling back to the full path if necessary.

fn default_name(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

/// Build a search query from optional filter parameters.
///
/// Assembles the various optional filter parameters into a complete Query
/// structure for the font search engine. Handles parsing of tag lists, ranges,
/// and other criteria into their proper internal representations.
#[allow(clippy::too_many_arguments)]
fn build_query(
    axes: Option<Vec<String>>,
    features: Option<Vec<String>>,
    scripts: Option<Vec<String>>,
    tables: Option<Vec<String>>,
    names: Option<Vec<String>>,
    codepoints: Option<Vec<String>>,
    text: Option<String>,
    weight: Option<String>,
    width: Option<String>,
    family_class: Option<String>,
    variable: bool,
) -> Result<Query> {
    // Parse all the tag lists - turning strings into proper typed tags
    let axes = parse_tag_list(&axes.unwrap_or_default())?;
    let features = parse_tag_list(&features.unwrap_or_default())?;
    let scripts = parse_tag_list(&scripts.unwrap_or_default())?;
    let tables = parse_tag_list(&tables.unwrap_or_default())?;
    let name_patterns = compile_patterns(&names.unwrap_or_default())?;

    // Handle the optional numeric filters - ranges can be tricky
    let weight_range = parse_optional_range(weight)?;
    let width_range = parse_optional_range(width)?;
    let family_class = parse_optional_family_class(family_class)?;

    // Mix explicit codepoints with any text characters provided
    let mut cps = parse_codepoints(&codepoints.unwrap_or_default())?;
    if let Some(text) = text {
        cps.extend(text.chars());
    }
    dedup_chars(&mut cps);

    // Assemble the final query with all our carefully parsed components
    Ok(Query::new()
        .with_axes(axes)
        .with_features(features)
        .with_scripts(scripts)
        .with_tables(tables)
        .with_name_patterns(name_patterns)
        .with_codepoints(cps)
        .require_variable(variable)
        .with_weight_range(weight_range)
        .with_width_range(width_range)
        .with_family_class(family_class))
}

/// Parse string character specifications into Unicode characters.
///
/// Converts string-based character specifications (single chars, ranges, or
/// hex codes) into actual Unicode values for the search engine. Supports
/// multiple input formats for flexible character selection.

fn parse_codepoints(raw: &[String]) -> Result<Vec<char>> {
    let mut cps = Vec::new();
    for chunk in raw {
        cps.extend(parse_codepoint_list(chunk)?);
    }
    Ok(cps)
}

/// Parse optional numeric range from string input.
///
/// Handles conversion from string representation to inclusive numeric range.
/// Returns None for empty input, allowing callers to distinguish between
/// "no range specified" and "invalid range format".
fn parse_optional_range(raw: Option<String>) -> Result<Option<RangeInclusive<u16>>> {
    match raw {
        Some(value) => Ok(Some(parse_u16_range(&value)?)),
        None => Ok(None),
    }
}

/// Parse optional family class filter from string input.
///
/// Handles family class filter parsing with proper error handling.
/// Returns None for empty input, maintaining consistency with other
/// optional parameter parsers.
fn parse_optional_family_class(raw: Option<String>) -> Result<Option<FamilyClassFilter>> {
    match raw {
        Some(value) => Ok(Some(parse_family_class(&value)?)),
        None => Ok(None),
    }
}

/// Compile string patterns into regex objects for name matching.
///
/// Transforms pattern strings into compiled regular expressions for
/// efficient font name filtering. Provides detailed error messages
/// for invalid regex syntax to help developers debug patterns.
fn compile_patterns(patterns: &[String]) -> Result<Vec<Regex>> {
    patterns
        .iter()
        .map(|p| Regex::new(p).map_err(|e| anyhow!("invalid regex {p}: {e}")))
        .collect()
}

/// Remove duplicate characters from the codepoint list.
///
/// Deduplicates the character list to optimize search performance and
/// avoid redundant matches. Sorts characters first for efficient
/// deduplication using the vector's built-in method.
fn dedup_chars(cps: &mut Vec<char>) {
    cps.sort();
    cps.dedup();
}

/// Convert internal font match structures to Python dictionaries.
///
/// Transforms Rust's TypgFontFaceMatch structures into Python dictionary
/// objects for easy consumption by Python code. Handles conversion of
/// typed fields to appropriate Python types and maintains nested structure.
fn to_py_matches(py: Python<'_>, matches: Vec<TypgFontFaceMatch>) -> PyResult<Vec<Py<PyAny>>> {
    matches
        .into_iter()
        .map(|item| {
            let meta = &item.metadata;

            let meta_dict = PyDict::new(py);
            meta_dict.set_item("names", meta.names.clone())?;
            meta_dict.set_item(
                "axis_tags",
                meta.axis_tags
                    .iter()
                    .map(|t| tag_to_string(*t))
                    .collect::<Vec<_>>(),
            )?;
            meta_dict.set_item(
                "feature_tags",
                meta.feature_tags
                    .iter()
                    .map(|t| tag_to_string(*t))
                    .collect::<Vec<_>>(),
            )?;
            meta_dict.set_item(
                "script_tags",
                meta.script_tags
                    .iter()
                    .map(|t| tag_to_string(*t))
                    .collect::<Vec<_>>(),
            )?;
            meta_dict.set_item(
                "table_tags",
                meta.table_tags
                    .iter()
                    .map(|t| tag_to_string(*t))
                    .collect::<Vec<_>>(),
            )?;
            meta_dict.set_item(
                "codepoints",
                meta.codepoints
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>(),
            )?;
            meta_dict.set_item("is_variable", meta.is_variable)?;
            meta_dict.set_item("weight_class", meta.weight_class)?;
            meta_dict.set_item("width_class", meta.width_class)?;
            meta_dict.set_item("family_class", meta.family_class)?;

            let outer = PyDict::new(py);
            outer.set_item("path", item.source.path.to_string_lossy().to_string())?;
            outer.set_item("ttc_index", item.source.ttc_index)?;
            outer.set_item("metadata", meta_dict)?;

            Ok(outer.into_any().unbind())
        })
        .collect()
}

/// Convert Rust error types to Python ValueError exceptions.
///
/// Transforms Rust's anyhow::Error into Python's ValueError with a
/// readable message. This bridge function ensures error information
/// flows correctly from Rust to Python while maintaining stack traces.

fn to_py_err(err: anyhow::Error) -> PyErr {
    PyValueError::new_err(err.to_string())
}

/// Register all Python-exposed functions with the module.
///
/// Creates the Python module interface by registering each function with
/// appropriate names. Conditionally includes indexed search functions
/// when the hpindex feature flag is enabled at compile time.
#[pymodule]
#[pyo3(name = "_typg_python")]
fn typg_python(_py: Python<'_>, m: &Bound<PyModule>) -> PyResult<()> {
    // Core search functions - always available
    m.add_function(wrap_pyfunction!(find_py, m)?)?;
    m.add_function(wrap_pyfunction!(find_paths_py, m)?)?;
    m.add_function(wrap_pyfunction!(filter_cached_py, m)?)?;

    // Indexed search functions - requires hpindex feature flag
    #[cfg(feature = "hpindex")]
    {
        m.add_function(wrap_pyfunction!(find_indexed_py, m)?)?;
        m.add_function(wrap_pyfunction!(list_indexed_py, m)?)?;
        m.add_function(wrap_pyfunction!(count_indexed_py, m)?)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(path: &str, names: &[&str], axes: &[&str], variable: bool) -> MetadataInput {
        MetadataInput {
            path: PathBuf::from(path),
            names: names.iter().map(|s| s.to_string()).collect(),
            axis_tags: axes.iter().map(|s| s.to_string()).collect(),
            feature_tags: Vec::new(),
            script_tags: Vec::new(),
            table_tags: Vec::new(),
            codepoints: vec!["A".into()],
            is_variable: variable,
            ttc_index: None,
            weight_class: None,
            width_class: None,
            family_class: None,
        }
    }

    #[test]
    fn filter_cached_filters_axes_and_names() {
        Python::initialize();
        Python::attach(|py| {
            let entries = vec![
                metadata("VariableVF.ttf", &["Pro VF"], &["wght"], true),
                metadata("Static.ttf", &["Static Sans"], &[], false),
            ];

            let result = filter_cached_py(
                py,
                entries,
                Some(vec!["wght".into()]),
                None,
                None,
                None,
                Some(vec!["Pro".into()]),
                None,
                None,
                None,
                None,
                None,
                true,
            );

            assert!(result.is_ok(), "expected Ok from filter_cached_py");
            let objs = result.unwrap();
            assert_eq!(objs.len(), 1, "only variable font with axis should match");
            let py_any: Py<PyAny> = objs[0].clone_ref(py);
            let bound_any = py_any.bind(py);
            let dict = bound_any.downcast::<PyDict>().unwrap();
            assert_eq!(
                dict.get_item("path")
                    .expect("path lookup")
                    .expect("path field")
                    .extract::<String>()
                    .unwrap(),
                "VariableVF.ttf"
            );
        });
    }

    #[test]
    fn invalid_tag_returns_error() {
        Python::initialize();
        Python::attach(|py| {
            let err = filter_cached_py(
                py,
                vec![metadata("Bad.ttf", &["Bad"], &["wght"], true)],
                Some(vec!["abcde".into()]),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                false,
            )
            .unwrap_err();

            let message = format!("{err}");
            assert!(
                message.contains("tag") || message.contains("invalid"),
                "error message should mention invalid tag, got: {message}"
            );
        });
    }

    #[test]
    fn find_requires_paths() {
        Python::initialize();
        Python::attach(|py| {
            let err = find_py(
                py,
                Vec::new(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                false,
                false,
                None,
            )
            .unwrap_err();

            assert!(
                format!("{err}").contains("path"),
                "should mention missing paths"
            );
        });
    }

    #[test]
    fn find_paths_requires_paths() {
        Python::initialize();
        Python::attach(|_| {
            let err = find_paths_py(
                Vec::new(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                false,
                false,
                None,
            )
            .unwrap_err();

            assert!(format!("{err}").contains("path"));
        });
    }

    #[cfg(feature = "hpindex")]
    #[test]
    fn indexed_search_returns_results() {
        use read_fonts::types::Tag;
        use std::time::SystemTime;
        use tempfile::TempDir;
        use typg_core::index::FontIndex;

        // Keep TempDir alive for the entire test.
        let dir = TempDir::new().unwrap();
        let index_path = dir.path().to_path_buf();

        // Create an index with a mock font entry.
        {
            let index = FontIndex::open(&index_path).unwrap();
            let mut writer = index.writer().unwrap();
            writer
                .add_font(
                    Path::new("/test/IndexedFont.ttf"),
                    None,
                    SystemTime::UNIX_EPOCH,
                    vec!["Indexed Font".to_string()],
                    &[Tag::new(b"wght")],
                    &[Tag::new(b"smcp")],
                    &[Tag::new(b"latn")],
                    &[],
                    &['a', 'b', 'c'],
                    true,
                    Some(400),
                    Some(5),
                    None,
                )
                .unwrap();
            writer.commit().unwrap();
        }

        // Test the bindings (dir is still alive here).
        Python::initialize();
        Python::attach(|py| {
            // Test count_indexed_py.
            let count = count_indexed_py(index_path.clone()).unwrap();
            assert_eq!(count, 1);

            // Test list_indexed_py.
            let all = list_indexed_py(py, index_path.clone()).unwrap();
            assert_eq!(all.len(), 1);

            // Test find_indexed_py with matching filter.
            let matches = find_indexed_py(
                py,
                index_path.clone(),
                Some(vec!["wght".into()]),
                Some(vec!["smcp".into()]),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                true,
            )
            .unwrap();
            assert_eq!(matches.len(), 1);

            // Test find_indexed_py with non-matching filter.
            let no_matches = find_indexed_py(
                py,
                index_path.clone(),
                None,
                Some(vec!["liga".into()]), // Not in the index
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                false,
            )
            .unwrap();
            assert_eq!(no_matches.len(), 0);
        });

        // dir is dropped here, after all tests complete.
    }
}
