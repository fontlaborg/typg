# typg Changelog

made by FontLab https://www.fontlab.com/

## v0.0.0 (unreleased)
- Initialized planning docs (PLAN, WORK, TODO updates) and recorded reuse/metrics direction for typg.
- Documented no direct dependency on fontgrep/fontgrepc, added typg-core workspace crate with initial filesystem discovery helper plus tests.
- Implemented query/filter parsing with property tests, added baseline search pipeline with metadata extraction and JSON/NDJSON writers in typg-core.
- Added cached-metadata filter path in typg-core to support future cache indexes without filesystem IO.
- Scaffolded typg-cli with clap-based `find` subcommand and output selection (plain/JSON/NDJSON).
- Extended `typg-cli find` with `--text`, STDIN path intake, and `--system-fonts` discovery, plus helper tests and env override for deterministic system font roots.
- Hardened discovery/output by testing nested directories, symlink following, and NDJSON line formatting; expanded CLI help/columns coverage with colorized output defaults.
- Added columnar + colorized text output to `typg-cli find`, with help snapshots and flag parsing coverage.
- Introduced criterion bench `codepoints` to compare typg-core codepoint parsing against the original fontgrep implementation.
- Added `typg-python` crate with PyO3 bindings for `find`/cached filtering, Fire-based `typgpy` CLI wrapper, maturin `pyproject.toml`, and Rust-side binding tests to seed Python parity work.
- Aligned publish pipeline to git tags via hatch-vcs, renamed the PyPI package to `typg`, and synced Cargo crate versions/dependency constraints for crates.io releases.
- Swapped the Rust CLI binary name to `typg`, fixed build/publish scripts for git-tag semver + tokenized publishing, and added a GitHub Actions release flow that builds cross-platform wheels, publishes crates, and attaches artifacts on `vN.N.N` tags.
- Dropped the fontgrep dev-dependency/criterion bench, fixed TTC suffix rendering in CLI plain output tests, and verified `cargo test --workspace` runs without linked/fontgrep present.
- Expanded README with CLI/Python/Rust usage examples plus migration guidance for fontgrep/fontgrepc users; deepened ARCHITECTURE.md with reuse/limitation notes.
- Refreshed CI to mirror typf/fontsimi layout (lint gate, cross-OS Rust tests, Python bindings check).
- Captured live-scan microbench on `typf/test-fonts` (9 faces; 20 runs: mean 30.6 ms, min 6.5 ms, max 226 ms) and documented current limitations.
- Added pytest coverage for typg-python (variable filter + system font env override) using shared test fonts.
- Added rayon-backed `--jobs` search parallelism (Rust + Python bindings), CLI integration tests against typf/test-fonts, and Python/Fire CLIs that accept the jobs knob.
- Added JSON cache support with `typg cache add/list/find/clean`, dedup/clean handling, and integration tests exercising cached search over typf/test-fonts.
- Added `--paths` output for `find` and cache commands plus Python `find_paths`/`paths_only` support to feed typf/fontlift/testypf pipelines without parsing JSON.
- Added optional HTTP server (`typg serve`) exposing `/health` and `/search` (JSON or paths-only) for remote querying.
- Added OS/2 weight/width shorthands across core, CLI/cache, Python bindings, and HTTP server with new unit/integ tests; documented flags and silenced clippy noise on binding entrypoints.
- Added validation polish: Axum tests for `/search` bad requests, CLI regression test keeping `--paths` output ANSI-free even with forced color, and Python coverage for `find_paths`/`paths_only` surfaces.
- Added OS/2 family-class filtering (major or `major.subclass`, with aliases like `sans`/`ornamental`/`script`) across core, CLI/cache, HTTP, and Python bindings; updated README/spec/examples and added unit/property coverage.
- Ignored Python build artifacts/wheels/pycache and removed stray generated binaries from the working tree to keep commits clean.
- Ingest name-table strings (family/typographic/full/PostScript) into metadata, deduplicate/sort tags/codepoints/names for deterministic cache/CLI output, and point integration fixtures at shared test fonts with name-filter regression coverage.
- **Phase 11: High-Performance Embedded Index (hpindex feature)**:
  - Added LMDB-backed index via `heed` crate for O(K) query performance on large font collections.
  - Implemented Roaring Bitmaps for ultra-fast tag intersection queries.
  - Added bincode serialization for font metadata storage.
  - Implemented incremental updates via xxhash path hashing and mtime comparison.
  - CLI: `--index` and `--index-path` flags on `cache add/find/list/clean` commands.
  - Python bindings: `find_indexed()`, `list_indexed()`, `count_indexed()` functions.
  - HTTP server: `/search` endpoint now supports `use_index` and `index_path` fields.
  - Added criterion benchmark comparing live scan vs LMDB index (~5x speedup on test fonts).
  - Build with `cargo build --features hpindex` to enable.
- **CLI polish**:
  - Added global `--quiet` / `-q` flag to suppress informational stderr messages for scripting use.
  - Added `cache info` subcommand to show cache/index statistics (path, type, font count, size in bytes).
  - Added `--count` flag to `cache find` to output only the number of matching fonts (useful for scripting).
  - `cache info` supports `--json` output and `--index` flag for LMDB index stats.
