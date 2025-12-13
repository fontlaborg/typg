//! HTTP server for typg - like a friendly librarian for fonts (made by FontLab https://www.fontlab.com/)
//!
//! This module serves up font search capabilities through a cozy little web API.
//! Think of it as the front desk for your typographic adventures, welcoming
//! requests and finding the perfect font matches faster than you can say "serif".

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

/// A gentle request to find fonts that match your wildest typographic dreams.
///
/// This struct captures all the parameters for a font search expedition.
/// It's like leaving a detailed note for the font fairy: "I want something
/// with Latin script, variable weight, and maybe a touch of elegance..."
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SearchRequest {
    /// File paths where fonts might be hiding
    pub paths: Vec<PathBuf>,
    /// Variable font axes you'd like to explore (weight, width, optical size, etc.)
    pub axes: Vec<String>,
    /// OpenType features that make fonts dance (liga, dlig, calt, etc.)
    pub features: Vec<String>,
    /// Scripts you need to support (latn, arab, cyrl, hani, etc.)
    pub scripts: Vec<String>,
    /// Font tables you care about (GDEF, GSUB, GPOS, etc.)
    pub tables: Vec<String>,
    /// Font names or family names you're looking for
    pub names: Vec<String>,
    /// Specific Unicode characters that must be present
    pub codepoints: Vec<String>,
    /// Sample text to test font compatibility
    pub text: Option<String>,
    /// Only include fonts that can do the variable font shimmy
    pub variable: bool,
    /// Follow symbolic links like a curious kitten
    pub follow_symlinks: bool,
    /// Number of parallel font explorers to send on the quest
    pub jobs: Option<usize>,
    /// Just return paths, not the full font metadata
    pub paths_only: bool,
    /// Specific weight class you're craving
    pub weight: Option<String>,
    /// Width class preference (compressed, extended, etc.)
    pub width: Option<String>,
    /// Font family class (serif, sans-serif, script, etc.)
    pub family_class: Option<String>,
    /// Use LMDB index instead of live scan (requires hpindex feature)
    /// This is like using a map instead of wandering around asking for directions
    pub use_index: bool,
    /// Custom index path (defaults to ~/.cache/typg/index or TYPOG_INDEX_PATH)
    /// Your personal font library card catalog
    pub index_path: Option<PathBuf>,
}

/// The treasure chest overflowing with font discoveries!
///
/// The server sends this back when it finds fonts that match your request.
/// It's like a happy little box of typographic surprises - either full font
/// details with all their charming metadata, or just the file paths if you asked
/// for the Cliff's Notes version.
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    /// Full font match details when you want the whole story
    pub matches: Option<Vec<TypgFontFaceMatch>>,
    /// File paths only when you're in a hurry and just need addresses
    pub paths: Option<Vec<String>>,
}

/// Opens the doors to the font search cafe and starts serving requests.
///
/// This function launches an HTTP server that listens for font search requests.
/// It's like setting up a cozy little shop where people come asking for fonts,
/// and we help them find exactly what they need with a smile and some fast responses.
pub async fn serve(bind: &str) -> Result<()> {
    // Set up our welcoming door where visitors can knock
    let listener = TcpListener::bind(bind)
        .await
        .with_context(|| format!("binding HTTP server to {bind}"))?;

    // Start serving up font-finding goodness to all who ask
    axum::serve(listener, router())
        .await
        .context("serving HTTP")?;
    Ok(())
}

/// Creates the road map for our tiny HTTP adventure.
///
/// This function builds the routing table that directs incoming requests
/// to the right handlers. It's like a friendly receptionist who knows exactly
/// where to send everyone - health checks to the wellness checkup room,
/// font searches to the typographic treasure hunt department.
pub fn router() -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/search", post(search_handler))
}

/// The heart of our operation - where font dreams come true.
///
/// This handler takes font search requests and turns them into actual font matches.
/// It's like having a helpful librarian who listens to your vague description
/// ("I need something fancy but readable") and returns exactly the right books
/// from the vast library of typography.
async fn search_handler(
    Json(req): Json<SearchRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Index mode doesn't need paths (searches the index)
    // Like using the card catalog instead of wandering the aisles
    #[cfg(feature = "hpindex")]
    let needs_paths = !req.use_index;
    #[cfg(not(feature = "hpindex"))]
    let needs_paths = true;

    // Make sure we have somewhere to look for fonts
    if needs_paths && req.paths.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "at least one search path is required".to_string(),
        ));
    }

    // Can't have zero workers - that's like trying to clean the house with nobody
    if matches!(req.jobs, Some(0)) {
        return Err((
            StatusCode::BAD_REQUEST,
            "jobs must be at least 1 when provided".to_string(),
        ));
    }

    // Build the search query from all the lovely parameters
    let query = build_query_from_parts(
        &req.axes,
        &req.features,
        &req.scripts,
        &req.tables,
        &req.names,
        &req.codepoints,
        &req.text,
        req.variable,
        &req.weight,
        &req.width,
        &req.family_class,
    )
    .map_err(to_bad_request)?;

    // Dispatch to index search if requested (the fancy, fast way)
    #[cfg(feature = "hpindex")]
    if req.use_index {
        let index_path = resolve_index_path(&req.index_path).map_err(to_bad_request)?;
        let query_clone = query.clone();

        // Let the tokio fairies do the heavy lifting in the background
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

        // Format the response based on what the caller asked for
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

    // Set up the live search options (the adventurous way)
    let opts = SearchOptions {
        follow_symlinks: req.follow_symlinks,
        jobs: req.jobs,
    };

    // Clone everything for the background task (don't want to block the main thread!)
    let paths = req.paths.clone();
    let query_clone = query.clone();
    let opts_clone = opts.clone();

    // Send the font explorers on their quest while we wait patiently
    let matches = task::spawn_blocking(move || search(&paths, &query_clone, &opts_clone))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("task join error: {e}"),
            )
        })?
        .map_err(to_bad_request)?;

    // Wrap up the treasures in the requested format
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

/// Turns any sad error into a polite HTTP bad request response.
///
/// This helper function is like a gentle translator that speaks both
/// "error" and "HTTP" fluently, ensuring error messages reach the caller
/// with proper status codes and no hard feelings.
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

    /// Hunt for test fonts like a determined but gentle detective.
    ///
    /// This helper function searches far and wide for the test fonts directory,
    /// checking several common hiding spots like a patient parent looking for
    /// a misplaced teddy bear. Returns None if the fonts are playing hide and seek.
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

    /// Test that we can ask for just font paths without all the fancy details.
    ///
    /// This test sends a search request and asks for paths only, then checks
    /// if we get back a nice collection of file paths like a well-organized library.
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

    /// Make sure we politely ask for paths when someone forgets to provide them.
    ///
    /// This test ensures the server gently reminds users that they need to tell us
    /// where to look for fonts, like saying "you forgot to tell me where to search!"
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

    /// Ensure we don't let anyone try the impossible "zero workers" trick.
    ///
    /// This test verifies that asking for zero parallel workers gets a gentle
    /// but firm rejection - we can't clean the house with nobody helping!
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

    /// Check if our little cafe is open for business.
    ///
    /// This ensures the health endpoint responds with a cheerful "ok",
    /// like a friendly barista saying "we're open and ready to serve you!"
    #[tokio::test]
    async fn health_endpoint_returns_ok() {
        let app = router();
        let request = Request::get("/health").body(Body::empty()).unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body.as_ref(), b"ok");
    }

    /// Test the super-fast indexed search like a font-seeking superhero.
    ///
    /// This test builds a temporary index, fills it with fonts, then searches
    /// via HTTP to make sure the fancy indexing magic works properly. It's like
    /// creating a mini library catalog and then asking if we can find books with it!
    #[cfg(feature = "hpindex")]
    #[tokio::test]
    async fn search_endpoint_with_index() {
        use crate::resolve_index_path;
        use std::fs;
        use std::time::SystemTime;
        use typg_core::discovery::{FontDiscovery, PathDiscovery};
        use typg_core::index::FontIndex;
        use typg_core::search::{search, SearchOptions};
        use typg_core::query::Query;

        let fonts = match fonts_dir() {
            Some(dir) => dir,
            None => return, // skip when fixtures are unavailable
        };

        // Build a temporary index like a miniature font library
        let index_dir = tempfile::TempDir::new().unwrap();
        let index_path = index_dir.path().to_path_buf();

        // Discover and add fonts to index like a diligent librarian stamping books
        let discovery = PathDiscovery::new([fonts.clone()]);
        let font_sources = discovery.discover().unwrap();

        let all_matches = search(&[fonts.clone()], &Query::default(), &SearchOptions::default()).unwrap();

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

        // Now query via HTTP and see if our fancy index works
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
        assert!(!paths.is_empty(), "expected at least one result from index search");
    }
}
