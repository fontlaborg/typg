//! HTTP server for font search.
//!
//! Made by FontLab https://www.fontlab.com/

use std::path::PathBuf;

use anyhow::{Context, Result};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::task;
use typg_core::search::{search, SearchOptions, TypgFontFaceMatch};

#[cfg(feature = "hpindex")]
use typg_core::index::FontIndex;

use crate::build_query_from_parts;
#[cfg(feature = "hpindex")]
use crate::resolve_index_path;

/// Parameters for an HTTP font search request.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SearchRequest {
    /// Directories to search for fonts.
    pub paths: Vec<PathBuf>,
    /// Variable font axis tags to require (e.g. wght, wdth, opsz).
    pub axes: Vec<String>,
    /// OpenType feature tags to require (e.g. liga, dlig, calt).
    pub features: Vec<String>,
    /// Script tags to require (e.g. latn, arab, cyrl, hani).
    pub scripts: Vec<String>,
    /// Font table tags to require (e.g. GDEF, GSUB, GPOS).
    pub tables: Vec<String>,
    /// Name or family name patterns to match.
    pub names: Vec<String>,
    /// Regex patterns that must match creator fields (copyright, trademark, manufacturer, designer, description, URLs, license).
    pub creator: Vec<String>,
    /// Regex patterns that must match license fields (copyright, license description, license URL).
    pub license: Vec<String>,
    /// Unicode codepoints that must be present in the font.
    pub codepoints: Vec<String>,
    /// Sample text whose codepoints must all be present.
    pub text: Option<String>,
    /// Restrict results to variable fonts.
    pub variable: bool,
    /// Follow symbolic links during directory traversal.
    pub follow_symlinks: bool,
    /// Number of parallel worker threads.
    pub jobs: Option<usize>,
    /// Return file paths only, without full metadata.
    pub paths_only: bool,
    /// Required weight class value or range.
    pub weight: Option<String>,
    /// Required width class value or range.
    pub width: Option<String>,
    /// Required font family class (e.g. serif, sans-serif, script).
    pub family_class: Option<String>,
    /// Use the LMDB index instead of a live directory scan (requires hpindex feature).
    pub use_index: bool,
    /// Path to the LMDB index directory (defaults to ~/.cache/typg/index or TYPOG_INDEX_PATH).
    pub index_path: Option<PathBuf>,
}

/// Search result wrapper returned by the HTTP server.
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    /// Full font match details, present when `paths_only` is false.
    pub matches: Option<Vec<TypgFontFaceMatch>>,
    /// File paths only, present when `paths_only` is true.
    pub paths: Option<Vec<String>>,
}

/// Start the HTTP server bound to the given address.
pub async fn serve(bind: &str) -> Result<()> {
    let listener = TcpListener::bind(bind)
        .await
        .with_context(|| format!("binding HTTP server to {bind}"))?;

    axum::serve(listener, router())
        .await
        .context("serving HTTP")?;
    Ok(())
}

/// Build the router with `/health` and `/search` endpoints.
pub fn router() -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/search", post(search_handler))
}

/// Handle a POST `/search` request and return matching fonts.
async fn search_handler(
    Json(req): Json<SearchRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Index mode searches the LMDB index and does not require paths.
    #[cfg(feature = "hpindex")]
    let needs_paths = !req.use_index;
    #[cfg(not(feature = "hpindex"))]
    let needs_paths = true;

    if needs_paths && req.paths.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "at least one search path is required".to_string(),
        ));
    }

    if matches!(req.jobs, Some(0)) {
        return Err((
            StatusCode::BAD_REQUEST,
            "jobs must be at least 1 when provided".to_string(),
        ));
    }

    let query = build_query_from_parts(
        &req.axes,
        &req.features,
        &req.scripts,
        &req.tables,
        &req.names,
        &req.creator,
        &req.license,
        &req.codepoints,
        &req.text,
        req.variable,
        &req.weight,
        &req.width,
        &req.family_class,
    )
    .map_err(to_bad_request)?;

    #[cfg(feature = "hpindex")]
    if req.use_index {
        let index_path = resolve_index_path(&req.index_path).map_err(to_bad_request)?;
        let query_clone = query.clone();

        let matches = task::spawn_blocking(move || {
            let index = FontIndex::open(&index_path)?;
            let reader = index.reader()?;
            reader.find(&query_clone)
        })
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("task join error: {e}"),
            )
        })?
        .map_err(to_bad_request)?;

        return if req.paths_only {
            let paths: Vec<String> = matches.iter().map(|m| m.source.path_with_index()).collect();
            Ok(Json(SearchResponse {
                matches: None,
                paths: Some(paths),
            }))
        } else {
            Ok(Json(SearchResponse {
                matches: Some(matches),
                paths: None,
            }))
        };
    }

    #[cfg(not(feature = "hpindex"))]
    if req.use_index {
        return Err((
            StatusCode::BAD_REQUEST,
            "index search requires hpindex feature".to_string(),
        ));
    }

    let opts = SearchOptions {
        follow_symlinks: req.follow_symlinks,
        jobs: req.jobs,
    };

    let paths = req.paths.clone();
    let query_clone = query.clone();
    let opts_clone = opts.clone();

    let matches = task::spawn_blocking(move || search(&paths, &query_clone, &opts_clone))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("task join error: {e}"),
            )
        })?
        .map_err(to_bad_request)?;

    if req.paths_only {
        let paths: Vec<String> = matches.iter().map(|m| m.source.path_with_index()).collect();
        Ok(Json(SearchResponse {
            matches: None,
            paths: Some(paths),
        }))
    } else {
        Ok(Json(SearchResponse {
            matches: Some(matches),
            paths: None,
        }))
    }
}

/// Convert an error into a 400 Bad Request response.
fn to_bad_request(err: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use serde_json::json;
    use std::env;
    use tower::util::ServiceExt;

    /// Locate the test fonts directory, checking the environment variable and known relative paths.
    /// Returns `None` if no directory is found.
    fn fonts_dir() -> Option<PathBuf> {
        if let Ok(env_override) = env::var("TYPF_TEST_FONTS") {
            let path = PathBuf::from(env_override);
            if let Ok(dir) = path.canonicalize() {
                return Some(dir);
            }
        }

        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let candidates = [
            manifest_dir
                .join("..")
                .join("..")
                .join("typf")
                .join("test-fonts"),
            manifest_dir
                .join("..")
                .join("linked")
                .join("typf")
                .join("test-fonts"),
            manifest_dir.join("..").join("..").join("test-fonts"),
        ];

        for candidate in candidates {
            if let Ok(dir) = candidate.canonicalize() {
                return Some(dir);
            }
        }

        None
    }

    /// Verify that a search request with `paths_only: true` returns file paths and no match details.
    #[tokio::test]
    async fn search_endpoint_returns_paths_only() {
        let fonts = match fonts_dir() {
            Some(dir) => dir,
            None => return, // skip when fixtures are unavailable
        };

        let app = router();
        let payload = json!({
            "paths": [fonts],
            "scripts": ["latn"],
            "paths_only": true,
            "jobs": 1
        });

        let request = Request::post("/search")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let parsed: SearchResponse = serde_json::from_slice(&body).expect("parse response");
        let paths = parsed.paths.expect("paths response present");
        assert!(paths.iter().any(|p| p.ends_with("NotoSans-Regular.ttf")));
    }

    /// Verify that a search request with no paths returns 400 Bad Request.
    #[tokio::test]
    async fn search_endpoint_requires_paths() {
        let app = router();
        let payload = json!({"paths": [], "scripts": ["latn"]});

        let request = Request::post("/search")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8(body.to_vec()).expect("utf8 body");
        assert!(
            text.contains("at least one search path is required"),
            "body: {text}"
        );
    }

    /// Verify that `jobs: 0` returns 400 Bad Request.
    #[tokio::test]
    async fn search_endpoint_rejects_zero_jobs() {
        let app = router();
        let payload = json!({"paths": ["/tmp"], "jobs": 0});

        let request = Request::post("/search")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8(body.to_vec()).expect("utf8 body");
        assert!(text.contains("jobs must be at least 1"), "body: {text}");
    }

    /// Verify that `GET /health` returns 200 OK with body `"ok"`.
    #[tokio::test]
    async fn health_endpoint_returns_ok() {
        let app = router();
        let request = Request::get("/health").body(Body::empty()).unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body.as_ref(), b"ok");
    }

    /// Build a temporary LMDB index, populate it with test fonts, and verify the HTTP search endpoint returns results.
    #[cfg(feature = "hpindex")]
    #[tokio::test]
    async fn search_endpoint_with_index() {
        use crate::resolve_index_path;
        use std::fs;
        use std::time::SystemTime;
        use typg_core::discovery::{FontDiscovery, PathDiscovery};
        use typg_core::index::FontIndex;
        use typg_core::query::Query;
        use typg_core::search::{search, SearchOptions};

        let fonts = match fonts_dir() {
            Some(dir) => dir,
            None => return, // skip when fixtures are unavailable
        };

        let index_dir = tempfile::TempDir::new().unwrap();
        let index_path = index_dir.path().to_path_buf();

        let discovery = PathDiscovery::new([fonts.clone()]);
        let font_sources = discovery.discover().unwrap();

        let all_matches = search(
            &[fonts.clone()],
            &Query::default(),
            &SearchOptions::default(),
        )
        .unwrap();

        let index = FontIndex::open(&index_path).unwrap();
        let mut writer = index.writer().unwrap();
        for m in &all_matches {
            let mtime = fs::metadata(&m.source.path)
                .and_then(|meta| meta.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let _ = writer.add_font(
                &m.source.path,
                m.source.ttc_index,
                mtime,
                m.metadata.names.clone(),
                &m.metadata.axis_tags,
                &m.metadata.feature_tags,
                &m.metadata.script_tags,
                &m.metadata.table_tags,
                &m.metadata.codepoints.iter().copied().collect::<Vec<_>>(),
                m.metadata.is_variable,
                m.metadata.weight_class,
                m.metadata.width_class,
                m.metadata.family_class,
            );
        }
        writer.commit().unwrap();
        drop(index);

        let app = router();
        let payload = json!({
            "use_index": true,
            "index_path": index_path,
            "scripts": ["latn"],
            "paths_only": true
        });

        let request = Request::post("/search")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let parsed: SearchResponse = serde_json::from_slice(&body).expect("parse response");
        let paths = parsed.paths.expect("paths response present");
        assert!(
            !paths.is_empty(),
            "expected at least one result from index search"
        );
    }
}
