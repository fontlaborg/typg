//! A gentle comparison: methodical scanning vs remembered facts
//! 
//! Picture two librarians - one methodically checks every book each time
//! you ask about fonts, while the other keeps perfect notes and answers
//! instantly. Both get you there, but one prefers taking the scenic route.
//! 
//! The indexed approach usually finishes its tea before the live scan
//! finds the first book, but both methods have their charms. This benchmark
//! measures the difference in milliseconds, though we appreciate both approaches.
//! 
//! Run with: cargo bench --features hpindex -p typg-core
//! 
//! For meaningful comparisons, point TYPF_TEST_FONTS at a directory with
//! 100+ fonts - more books make for a more interesting comparison.
//! 
//! Crafted with curiosity at FontLab https://www.fontlab.com/

use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tempfile::TempDir;
use typg_core::discovery::{FontDiscovery, PathDiscovery};
use typg_core::index::FontIndex;
use typg_core::query::Query;
use typg_core::search::{search, SearchOptions};
use typg_core::tags::tag4;

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

/// The methodical approach: searching fonts with fresh eyes each time
/// 
/// Like a librarian who carefully examines every book for each request,
/// this method reads font files directly every single time you ask.
/// Unfailingly accurate and wonderfully thorough, though it prefers
/// taking the scenic route through every byte of the filesystem.
fn bench_live_scan(c: &mut Criterion) {
    let fonts = match fonts_dir() {
        Some(dir) => dir,
        None => {
            eprintln!("Skipping live_scan benchmark: test fonts not found");
            return;
        }
    };

    let query = Query::new()
        .with_scripts(vec![tag4("latn").unwrap()])
        .with_features(vec![tag4("kern").unwrap()]);
    let opts = SearchOptions::default();
    let paths = vec![fonts];

    c.bench_function("live_scan_latn_kern", |b| {
        b.iter(|| {
            let matches = search(black_box(&paths), black_box(&query), &opts).unwrap();
            black_box(matches.len())
        })
    });
}

/// The remembered approach: instant answers from prepared knowledge
/// 
/// Like a librarian with perfect notes and excellent organization skills,
/// this index remembers what it learned earlier and answers queries instantly.
/// Complex questions become simple lookups, with responses arriving before
/// you've finished your morning tea. It's having a knowledgeable assistant
/// who's already done the reading and kept excellent notes.
fn bench_lmdb_index(c: &mut Criterion) {
    let fonts = match fonts_dir() {
        Some(dir) => dir,
        None => {
            eprintln!("Skipping lmdb_index benchmark: test fonts not found");
            return;
        }
    };

    // Build index first
    let index_dir = TempDir::new().unwrap();
    let index_path = index_dir.path().to_path_buf();

    // Discover fonts and add to index
    let discovery = PathDiscovery::new([fonts.clone()]);
    let font_sources = discovery.discover().unwrap();

    let index = FontIndex::open(&index_path).unwrap();
    let mut writer = index.writer().unwrap();

    for source in &font_sources {
        let mtime = fs::metadata(&source.path)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        // Get metadata via search for this single font
        if let Ok(matches) = search(&[source.path.clone()], &Query::default(), &SearchOptions::default()) {
            for m in matches {
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
        }
    }
    writer.commit().unwrap();
    drop(index);

    eprintln!("Index built with {} font sources", font_sources.len());

    // Benchmark index query
    let query = Query::new()
        .with_scripts(vec![tag4("latn").unwrap()])
        .with_features(vec![tag4("kern").unwrap()]);

    c.bench_function("lmdb_index_latn_kern", |b| {
        b.iter(|| {
            let index = FontIndex::open(black_box(&index_path)).unwrap();
            let reader = index.reader().unwrap();
            let matches = reader.find(black_box(&query)).unwrap();
            black_box(matches.len())
        })
    });
}

criterion_group!(benches, bench_live_scan, bench_lmdb_index);
criterion_main!(benches);
