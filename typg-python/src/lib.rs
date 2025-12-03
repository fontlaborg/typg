//! PyO3 bindings for typg-core (made by FontLab https://www.fontlab.com/)

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
    #[pyo3(default)]
    weight_class: Option<u16>,
    #[pyo3(default)]
    width_class: Option<u16>,
    #[pyo3(default)]
    family_class: Option<u16>,
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
    if paths.is_empty() {
        return Err(PyValueError::new_err(
            "at least one search path is required",
        ));
    }

    if matches!(jobs, Some(0)) {
        return Err(PyValueError::new_err(
            "jobs must be at least 1 when provided",
        ));
    }

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

    let opts = SearchOptions {
        follow_symlinks,
        jobs,
    };
    let matches = search(&paths, &query, &opts).map_err(to_py_err)?;
    to_py_matches(py, matches)
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
    if paths.is_empty() {
        return Err(PyValueError::new_err(
            "at least one search path is required",
        ));
    }

    if matches!(jobs, Some(0)) {
        return Err(PyValueError::new_err(
            "jobs must be at least 1 when provided",
        ));
    }

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

    let opts = SearchOptions {
        follow_symlinks,
        jobs,
    };
    let matches = search(&paths, &query, &opts).map_err(to_py_err)?;
    Ok(matches
        .into_iter()
        .map(|m| m.source.path_with_index())
        .collect())
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
    let metadata = convert_metadata(entries).map_err(to_py_err)?;
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

    let matches = filter_cached(&metadata, &query);
    to_py_matches(py, matches)
}

fn convert_metadata(entries: Vec<MetadataInput>) -> Result<Vec<TypgFontFaceMatch>> {
    entries
        .into_iter()
        .map(|entry| {
            let mut names = entry.names;
            if names.is_empty() {
                names.push(default_name(&entry.path));
            }

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
    weight: Option<String>,
    width: Option<String>,
    family_class: Option<String>,
    variable: bool,
) -> Result<Query> {
    let axes = parse_tag_list(&axes.unwrap_or_default())?;
    let features = parse_tag_list(&features.unwrap_or_default())?;
    let scripts = parse_tag_list(&scripts.unwrap_or_default())?;
    let tables = parse_tag_list(&tables.unwrap_or_default())?;
    let name_patterns = compile_patterns(&names.unwrap_or_default())?;
    let weight_range = parse_optional_range(weight)?;
    let width_range = parse_optional_range(width)?;
    let family_class = parse_optional_family_class(family_class)?;

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
        .require_variable(variable)
        .with_weight_range(weight_range)
        .with_width_range(width_range)
        .with_family_class(family_class))
}

fn parse_codepoints(raw: &[String]) -> Result<Vec<char>> {
    let mut cps = Vec::new();
    for chunk in raw {
        cps.extend(parse_codepoint_list(chunk)?);
    }
    Ok(cps)
}

fn parse_optional_range(raw: Option<String>) -> Result<Option<RangeInclusive<u16>>> {
    match raw {
        Some(value) => Ok(Some(parse_u16_range(&value)?)),
        None => Ok(None),
    }
}

fn parse_optional_family_class(raw: Option<String>) -> Result<Option<FamilyClassFilter>> {
    match raw {
        Some(value) => Ok(Some(parse_family_class(&value)?)),
        None => Ok(None),
    }
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
            meta_dict.set_item(
                "family_class",
                meta.family_class.map(|(class, subclass)| (class, subclass)),
            )?;

            let outer = PyDict::new(py);
            outer.set_item("path", item.source.path.to_string_lossy().to_string())?;
            outer.set_item("ttc_index", item.source.ttc_index)?;
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
    m.add_function(wrap_pyfunction!(find_paths_py, m)?)?;
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
}
