//! PyO3 bindings for typg-core (made by FontLab https://www.fontlab.com/)

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use regex::Regex;
use typg_core::query::{parse_codepoint_list, parse_tag_list, Query};
use typg_core::search::{filter_cached, search, FontMatch, FontMetadata, SearchOptions};
use typg_core::tags::tag_to_string;

#[derive(Clone, Debug, FromPyObject)]
struct MetadataInput {
    path: PathBuf,
    #[pyo3(default)]
    names: Vec<String>,
    #[pyo3(default)]
    axis_tags: Vec<String>,
    #[pyo3(default)]
    feature_tags: Vec<String>,
    #[pyo3(default)]
    script_tags: Vec<String>,
    #[pyo3(default)]
    table_tags: Vec<String>,
    #[pyo3(default)]
    codepoints: Vec<String>,
    #[pyo3(default)]
    is_variable: bool,
    #[pyo3(default)]
    ttc_index: Option<u32>,
}

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
        variable=false,
        follow_symlinks=false
    )
)]
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
    variable: bool,
    follow_symlinks: bool,
) -> PyResult<Vec<Py<PyAny>>> {
    if paths.is_empty() {
        return Err(PyValueError::new_err(
            "at least one search path is required",
        ));
    }

    let query = build_query(
        axes, features, scripts, tables, names, codepoints, text, variable,
    )
    .map_err(to_py_err)?;

    let opts = SearchOptions { follow_symlinks };
    let matches = search(&paths, &query, &opts).map_err(to_py_err)?;
    to_py_matches(py, matches)
}

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
        variable=false
    )
)]
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
    variable: bool,
) -> PyResult<Vec<Py<PyAny>>> {
    let metadata = convert_metadata(entries).map_err(to_py_err)?;
    let query = build_query(
        axes, features, scripts, tables, names, codepoints, text, variable,
    )
    .map_err(to_py_err)?;

    let matches = filter_cached(&metadata, &query);
    to_py_matches(py, matches)
}

fn convert_metadata(entries: Vec<MetadataInput>) -> Result<Vec<FontMetadata>> {
    entries
        .into_iter()
        .map(|entry| {
            let mut names = entry.names;
            if names.is_empty() {
                names.push(default_name(&entry.path));
            }

            Ok(FontMetadata {
                path: entry.path,
                names,
                axis_tags: parse_tag_list(&entry.axis_tags)?,
                feature_tags: parse_tag_list(&entry.feature_tags)?,
                script_tags: parse_tag_list(&entry.script_tags)?,
                table_tags: parse_tag_list(&entry.table_tags)?,
                codepoints: parse_codepoints(&entry.codepoints)?,
                is_variable: entry.is_variable,
                ttc_index: entry.ttc_index,
            })
        })
        .collect()
}

fn default_name(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

fn build_query(
    axes: Option<Vec<String>>,
    features: Option<Vec<String>>,
    scripts: Option<Vec<String>>,
    tables: Option<Vec<String>>,
    names: Option<Vec<String>>,
    codepoints: Option<Vec<String>>,
    text: Option<String>,
    variable: bool,
) -> Result<Query> {
    let axes = parse_tag_list(&axes.unwrap_or_default())?;
    let features = parse_tag_list(&features.unwrap_or_default())?;
    let scripts = parse_tag_list(&scripts.unwrap_or_default())?;
    let tables = parse_tag_list(&tables.unwrap_or_default())?;
    let name_patterns = compile_patterns(&names.unwrap_or_default())?;

    let mut cps = parse_codepoints(&codepoints.unwrap_or_default())?;
    if let Some(text) = text {
        cps.extend(text.chars());
    }
    dedup_chars(&mut cps);

    Ok(Query::new()
        .with_axes(axes)
        .with_features(features)
        .with_scripts(scripts)
        .with_tables(tables)
        .with_name_patterns(name_patterns)
        .with_codepoints(cps)
        .require_variable(variable))
}

fn parse_codepoints(raw: &[String]) -> Result<Vec<char>> {
    let mut cps = Vec::new();
    for chunk in raw {
        cps.extend(parse_codepoint_list(chunk)?);
    }
    Ok(cps)
}

fn compile_patterns(patterns: &[String]) -> Result<Vec<Regex>> {
    patterns
        .iter()
        .map(|p| Regex::new(p).map_err(|e| anyhow!("invalid regex {p}: {e}")))
        .collect()
}

fn dedup_chars(cps: &mut Vec<char>) {
    cps.sort();
    cps.dedup();
}

fn to_py_matches(py: Python<'_>, matches: Vec<FontMatch>) -> PyResult<Vec<Py<PyAny>>> {
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
            meta_dict.set_item("ttc_index", meta.ttc_index)?;

            let outer = PyDict::new(py);
            outer.set_item("path", item.path.to_string_lossy().to_string())?;
            outer.set_item("metadata", meta_dict)?;

            Ok(outer.into_any().unbind())
        })
        .collect()
}

fn to_py_err(err: anyhow::Error) -> PyErr {
    PyValueError::new_err(err.to_string())
}

#[pymodule]
#[pyo3(name = "_typg_python")]
fn typg_python(_py: Python<'_>, m: &Bound<PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(find_py, m)?)?;
    m.add_function(wrap_pyfunction!(filter_cached_py, m)?)?;
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
                false,
                false,
            )
            .unwrap_err();

            assert!(
                format!("{err}").contains("path"),
                "should mention missing paths"
            );
        });
    }
}
