# typg Plan

made by FontLab https://www.fontlab.com/

## Scope (one sentence)
Build typg as an ultra-fast font search/discovery toolkit (Rust core + CLI + Python API) that matches fontgrep/fontgrepc capabilities while reusing typf/fontations assets wherever possible.

## Phase -1 — Housekeeping (status)
- [x] Create PLAN.md, WORK.md, and CHANGELOG.md for project tracking

## Phase 0 — Reuse inventory and early decisions

### typf repo (linked/typf)
- Crates to mine: `typf-core` (render pipeline types), `typf-fontdb` (font index/cache), `typf-unicode` (script detection), `typf-input` (path handling). Prefer reusing `typf-fontdb` indexes to avoid duplicating catalog builds.
- Decision: Inspect `typf-fontdb` APIs first for catalog ingestion; only fork if dependency footprint is too large for a lean search-only crate.

### fontgrep (linked/fontgrep)
- Provides battle-tested CLI syntax and filters (axes, features, scripts, tables, Unicode ranges, name regex, text coverage). Plan to mirror flag semantics and reuse parsing patterns.
- Decision: Copy/adapt CLI flag mapping and progressive output structure; avoid taking the crate as a direct dependency to keep control over query execution.

### fontgrepc (linked/fontgrepc)
- Adds SQLite-backed cache and bulk ingest (`add`, `find`, `list`, `clean`) with job parallelism. Useful reference for cache schema and cache-vs-live search UX.
- Decision: Borrow schema and cache management patterns; implement our own cache module to keep typg’s dependency tree minimal.

### fontations (linked/fontations)
- Core crates `read-fonts`, `write-fonts`, and `skrifa` deliver zero-copy parsing plus glyph/metadata access; match typg’s need for fast metadata extraction.
- Decision: Build typg-core on `read-fonts`/`skrifa`; keep `write-fonts` optional (only if we need patching/export flows).

### fontlift (linked/fontlift)
- Cross-platform font discovery/installation layers with OS-specific implementations and CLI. Valuable for enumerating system font roots cleanly on macOS/Windows.
- Decision: Reuse platform enumeration logic (without install/remove features) to seed search paths safely.

### Additional external crates
- `fontdb` offers a lightweight, cacheable font metadata index that can be prebuilt and queried without disk rescans; keep as fallback if typf-fontdb coupling is too heavy.

## Success metrics (initial)
- P50 search latency: ≤50 ms over 10k fonts on SSD; ≤250 ms over 100k-font cache.
- Feature parity: supports all fontgrep/fontgrepc query flags and JSON/NDJSON output.
- Catalog freshness: cache rebuild detects file changes within one run; no stale results after updates.
- Resource footprint: CLI binary ≤10 MB release build; memory ≤256 MB during a 100k-font query.

## Near-term steps (Phase 1 preview)
- [x] Catalog fontgrep/fontgrepc flags against desired typg behavior and capture in docs/spec.md.
- [x] Choose crate layout (`typg-core`, `typg-cli`, `typg-python`) and dependency boundaries.
- [x] Draft search use-cases (family/name, axes presence, glyph coverage, Unicode ranges, weight/class filters) and map to APIs.

## Phase 2 — Rust core (status)
- [x] Scaffolded `typg-core` with a filesystem discovery stub and tests to exercise font extension filtering.
- [x] Implemented query parsers and filter matching in `typg-core`.
- [x] Added baseline search pipeline (metadata extraction + streaming JSON/NDJSON output).
- [x] Documented why typf-fontdb is not yet used for caching (no persistent index, in-memory only) and added a cached-filter path in `typg-core` to accept precomputed metadata without touching the filesystem.

## Phase 3 — CLI (status)
- [~] `typg-cli` argument surface tracks fontgrep/fontgrepc for `find`; cache subcommands still pending.
- [x] Columnar/colorized output for `find` with JSON/NDJSON toggles, plus help coverage.

## Phase 3 — Rust CLI (status)
- [x] `typg-cli find` now accepts STDIN paths and a `--system-fonts` toggle while retaining recursive walk defaults; added `--text` filter to cover fontgrep parity.
- [~] Still to do: job controls and cache subcommands. Colorized/columnar output plus help/query snapshot coverage are in place.

## Phase 4 — Python bindings (status)
- [x] Added `typg-python` crate with PyO3 bindings that expose `find` and cached filtering, returning dict-friendly structures for Fire/CLI use.
- [x] Created `pyproject.toml` (maturin) and Fire-based CLI wrapper (`typgpy`) under `python/typg_python`.
- [~] Pytest coverage for the Python-facing API is still pending; Rust-side binding tests exist as a stopgap.

## Phase 5 — Docs & CI (status)
- [x] Updated README with overview/install/usage across Rust CLI, Python bindings, and library surfaces, plus migration guidance for fontgrep/fontgrepc users.
- [x] Expanded ARCHITECTURE.md to spell out data flow, typf/fontations reuse points, and current limitations.
- [x] Added CI workflow patterned after typf/fontsimi (lint gate, cross-OS Rust tests, Python binding build/tests).
- [x] Logged microbenchmarks and current limitations in WORK.md/CHANGELOG.md for traceability.
