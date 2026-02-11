# typg Plan

made by FontLab https://www.fontlab.com/

## Scope (one sentence)
Build typg as an ultra-fast font search/discovery toolkit (Rust core + CLI + Python API) that matches fontgrep/fontgrepc capabilities while reusing typf/fontations assets wherever possible.

## Phase -1 — Housekeeping (status)
- [x] Create TASKS.md, WORK.md, and CHANGELOG.md for project tracking

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
- [x] `typg-cli` argument surface tracks fontgrep/fontgrepc for `find`; cache subcommands now ship (JSON cache file with add/list/find/clean).
- [x] Columnar/colorized output for `find` with JSON/NDJSON toggles, plus help coverage.

## Phase 3 — Rust CLI (status)
- [x] `typg-cli find` now accepts STDIN paths and a `--system-fonts` toggle while retaining recursive walk defaults; added `--text` filter to cover fontgrep parity.
- [x] Cache subcommands (`add/list/find/clean`) now ingest/search JSON cache files; job controls land alongside cache ingest for parity.

## Phase 4 — Python bindings (status)
- [x] Added `typg-python` crate with PyO3 bindings that expose `find` and cached filtering, returning dict-friendly structures for Fire/CLI use.
- [x] Created `pyproject.toml` (maturin) and Fire-based CLI wrapper (`typgpy`) under `python/typg_python`.
- [x] Added pytest coverage for typg-python (live scan + system font env override) using shared test fonts.

## Phase 5 — Docs & CI (status)
- [x] Updated README with overview/install/usage across Rust CLI, Python bindings, and library surfaces, plus migration guidance for fontgrep/fontgrepc users.
- [x] Expanded ARCHITECTURE.md to spell out data flow, typf/fontations reuse points, and current limitations.
- [x] Added CI workflow patterned after typf/twasitors (lint gate, cross-OS Rust tests, Python binding build/tests).
- [x] Logged microbenchmarks and current limitations in WORK.md/CHANGELOG.md for traceability.

## Phase 6 — Integrations & Service (status)
- [x] Path-only output flags and Python `find_paths` helper added so typg results feed directly into typf/fontlift/testypf pipelines without post-processing.
- [x] Optional HTTP server (`typg serve`) exposes `/health` and `/search` (JSON or paths-only) for remote querying once core parity is achieved.

## Phase 7 — Parity polish (status)
- [x] Add OS/2 classification filters (weight and width) across core, CLI/cache, Python bindings, and HTTP server with tests.
- [x] Quiet clippy lint noise in typg-python by reducing argument lists (shared query params helper).
- [x] Harden validation and health checks with targeted tests (e.g., reject jobs=0, assert `/health` endpoint).

## Phase 8 — Validation polish (status)
- [x] Cover HTTP `/search` error paths (missing paths, jobs=0) with Axum tests.
- [x] Guard `--paths` output against ANSI when color is forced; add CLI test.
- [x] Exercise Python path-only surfaces (`find_paths`, `paths_only` flag in CLI) with fixtures.

## Phase 9 — Classification polish & hygiene (status)
- [x] Add OS/2 family-class filter (major + subclass, named aliases) across core/CLI/cache/HTTP/Python with tests.
- [x] Refresh docs/spec/README examples to show family-class usage.
- [x] Ignore Python build artifacts and remove stray compiled outputs from the repo.

## Phase 10 — Metadata polish (status)
- [x] Pull Unicode name-table entries (family/typo/full/PostScript) into search metadata so name regex filters hit real names instead of filenames.
- [x] Deduplicate and sort tags/codepoints/name lists for deterministic cache/CLI output.
- [x] Fix integration fixtures to find repo-level test fonts and add name-filter regression tests across CLI/core.

## Phase 11 — High-Performance Embedded Index (status)

**Objective:** Replace JSON cache with specialized embedded database for O(K) query performance on massive font collections (>100k fonts).

### Rationale
The JSON cache imposes O(N) linear scan on every query, parsing overhead at startup, and unbounded memory usage. This phase introduces:
- **LMDB via `heed`**: Memory-mapped, zero-copy reads, ACID transactions
- **Roaring Bitmaps**: Ultra-fast set intersections for tag queries
- **bincode serialization**: Fast, compact metadata storage (simplified from rkyv)
- **Roaring Bitmap cmap**: Deterministic Unicode coverage filtering (simplified from Cuckoo Filter)

### Expected Impact
- **Query speed**: 100x-1000x speedup for selective queries on large collections
- **Memory efficiency**: Usage independent of total font count
- **Scalability**: Millisecond responsiveness on 100k+ font libraries

### Architecture

#### Database Layout (LMDB Environment)
```
DB_METADATA:      FontID (u64) -> bincode<IndexedFontMeta>
DB_INVERTED_TAGS: Tag (u32)    -> RoaringBitmap<FontID>
DB_PATH_TO_ID:    PathHash     -> (FontID, mtime)
```

#### Ingestion Pipeline (`cache add --index`)
1. Use `heed` write transactions for atomic updates
2. Check `DB_PATH_TO_ID` for incremental updates via xxhash + mtime
3. Store Roaring Bitmap of cmap codepoints in metadata
4. Serialize metadata with bincode into `DB_METADATA`
5. Update Roaring Bitmaps in `DB_INVERTED_TAGS`
6. Run `RoaringBitmap::run_optimize()` before commit

#### Query Execution (`cache find --index`)
1. Use `heed` read transactions (non-blocking)
2. Retrieve bitmaps for query tags, perform intersection
3. Iterate result FontIDs, deserialize metadata via bincode
4. Apply numeric filters (weight/width/family-class) directly
5. For text queries, check Roaring Bitmap cmap coverage
6. Apply name regex on metadata strings

### Implementation Tasks (Completed)
- [x] Add `heed`, `roaring`, `bincode`, `bytemuck`, `xxhash-rust`, `byteorder` dependencies to `typg-core` (simplified from original plan)
- [x] Create `typg-core/src/index.rs` with core types (`FontID`, `FontIndex`, `IndexedFontMeta`)
- [x] Implement inverted index for tag → RoaringBitmap<FontID> mapping
- [x] Implement Roaring Bitmap for cmap coverage (simplified from Cuckoo Filter)
- [x] Implement `IndexWriter` for atomic ingestion pipeline with mtime-based incremental updates
- [x] Implement `IndexReader` for optimized query execution with bitmap intersection
- [x] Add `--index` and `--index-path` flags to CLI cache commands (`add`, `find`, `list`)
- [x] Maintain JSON cache as default fallback for backwards compatibility
- [x] Add unit tests for bitmap operations and integration tests for CLI

### Future Work
- [x] Add benchmarks comparing live scan vs Index performance (benches/cache_vs_index.rs)
- [x] Update Python bindings for indexed search (`find_indexed()`, `list_indexed()`, `count_indexed()`)
