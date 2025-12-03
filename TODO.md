# typg TODO

made by FontLab https://www.fontlab.com/

**Scope:** In this folder ( /Users/adam/Developer/vcs/github.fontlaborg/typg ) Build `typg`, a sister project to @./linked/typf/ , focused on ultra-fast font search/discovery with a Rust library, CLI, and Python API. 

## Phase -1 – Housekeeping

- [x] Create `PLAN.md` to capture scope, phases, and reuse decisions
- [x] Create `WORK.md` with a simple iteration log (date, goal, tests run)
- [x] Create `CHANGELOG.md` seeded with initial planning entry (v0.0.0, not released)

## Phase 0 – Groundwork

- [x] Inventory relevant crates inside @./linked/typf/ for potential reuse instead of reimplementing logic
- [x] Inventory and analyze @./linked/fontgrep/ and @./linked/fontgrepc/ for potential reuse instead of reimplementing logic
- [x] Inventory and analyze @./linked/fontations/ for potential reuse instead of reimplementing logic
- [x] Inventory and analyze @./linked/fontlift/ for potential reuse instead of reimplementing logic
- [x] Capture findings + tech choices in `PLAN.md` for typg 
- [x] Define success metrics (latency per query, supported filters, CLI parity with fontgrep/fontgrepc)

## Phase 1 – Source Analysis & Specification

- [x] Deep-dive @./linked/fontgrep/ and @./linked/fontgrepc/ to catalog every flag, filter, and output format
- [x] Don’t build a dependency on @./linked/fontgrep/ and @./linked/fontgrepc/ (just copy & adapt code from these two), but you may depend on their dependencies. 
- [x] Produce a comparison matrix (fontgrep/fontgrepc/typg) and store it in `docs/spec.md`
- [x] Specify crate layout (e.g., `typg-core`, `typg-cli`, `typg-python`) and dependency relationships
- [x] Document search use-cases (family name, axes presence, glyph coverage, Unicode range, weight/class filters)
- [x] Update @./CLAUDE.md so it’s specific to this project, explains briefly its objective and organization and coding style
- [x] Update @./README.md so that it describes the planned project in detail (what, how, why; how to install, how to use, how to contribute)

## Phase 2 – Rust Library
- [x] Scaffold `typg-core` crate with typf-compatible font discovery abstractions (build.rs to reuse fontdb indexes?)
- [x] Implement parsers for CLI query syntax (borrow from fontgrep) with property tests
- [x] Implement search pipeline: font loading → metadata extraction → filter evaluation → result ranking
- [x] Documented why typf-fontdb is not used yet (in-memory only, no persistent cache) and added a cached-filter hook in typg-core for precomputed metadata
- [x] Add streaming JSON/JSONL output plus structured Rust types
- [x] Write criterion benchmarks comparing against original fontgrep; target parity before refactors

## Phase 3 – Rust CLI
- [x] Build `typg-cli` crate with argument parser (clap/lexopt) mirroring fontgrep/fontgrepc options for `find`, including `--jobs`
- [x] `typg-cli` find subcommand scaffolded (axes/features/scripts/tables/name/codepoints/variable flags + json/ndjson/plain output)
- [x] Ensure CLI supports recursive directory walks, system font discovery, and STDIN/STDOUT piping
- [x] Implement colorized/columnar output plus `--json` / `--ndjson` toggles
- [x] Add snapshot tests for CLI help plus representative queries using `typf/test-fonts`
- [x] Add cache subcommands (`add/list/clean/find`) mirroring fontgrepc once cache module lands (JSON cache file, dedup + clean)

## Phase 4 – Python Bindings & Fire CLI
- [x] Design minimal PyO3 bindings that wrap `typg-core` search API (async-friendly if needed)
- [x] Provide `fire`-based CLI mirroring Rust CLI semantics (optionally Typer if richer UX needed)
- [x] Publish packaging metadata (pyproject via `maturin`) and document install flow in README
- [x] Add pytest suite hitting bindings + CLIs with golden files shared with Rust tests

## Phase 5 – Documentation & Verification
- [x] Update `README.md` with overview, install, usage examples (Rust, CLI, Python)
- [x] Create `ARCHITECTURE.md` describing data flow + reuse points from typf
- [x] Add CI workflow referencing typf + fontsimi pipelines
- [x] Add release workflow to publish crates (typg-core/typg-cli/typg-python) and PyPI wheels on semver tags
- [x] Document migration guidance for existing fontgrep/fontgrepc users
- [x] Record benchmarks + known limitations in `WORK.md` and `CHANGELOG.md`

## Phase 6 – Stretch
- [x] Explore integration hooks so typg can directly feed fonts into typf/fontlift/testypf workflows
- [x] Add optional gRPC/HTTP server mode for remote querying (only after core parity achieved)

## Phase 7 – Parity polish
- [x] Add OS/2 classification filters (weight + width) end-to-end (core, CLI/cache, Python, HTTP) with tests.
- [x] Quiet clippy lint noise in typg-python by consolidating query params to avoid `too_many_arguments`.
- [x] Harden validation and health checks with explicit tests (jobs=0 rejection, `/health` endpoint).

## Phase 8 – Validation polish

- [x] Add Axum tests for HTTP `/search` errors (no paths, jobs=0) to lock validation.
- [x] Ensure `--paths` output stays ANSI-free even when `--color always`; cover with CLI test.
- [x] Add Python tests for `find_paths` and CLI `paths_only` to guarantee path-only surfaces.

## Phase 9 – Classification & Hygiene
- [x] Add OS/2 family-class filtering (major + subclass aliases) across core/CLI/cache/HTTP/Python with tests.
- [x] Document family-class usage in README/spec/examples.
- [x] Ignore Python build artifacts and drop stray compiled outputs from the repo.

## Phase 10 – Metadata polish
- [x] Read Unicode name-table strings (family/typo/full/PostScript) into metadata so name regex filters match real names, not just filenames.
- [x] Deduplicate and sort metadata lists (tags, codepoints, names) for deterministic cache/CLI output.
- [x] Point integration fixtures at repo-level test fonts and add name-filter regression tests (core + CLI).

## Phase 11 – High-Performance Embedded Index

### Core Infrastructure
- [x] Add `heed`, `roaring`, `bytemuck`, `xxhash-rust`, `bincode`, `byteorder` dependencies to `typg-core/Cargo.toml` (simplified from original plan)
- [x] Create `typg-core/src/index.rs` module with core types:
  - [x] `FontID` (u64) for persistent font identification
  - [x] `FontIndex` struct holding LMDB environment and database handles
  - [x] `IndexedFontMeta` for serialized font metadata

### Inverted Index Implementation
- [x] Implement `DB_INVERTED_TAGS` database (Tag → RoaringBitmap<FontID>)
- [x] Add methods for inserting tags during ingestion
- [x] Add methods for bitmap intersection during queries

### Unicode Coverage Filtering
- [x] Implement Roaring Bitmap for cmap coverage (simplified from Cuckoo Filter)
- [x] Store serialized Roaring Bitmap in `DB_METADATA`
- [x] Add fast-path codepoint checking during queries

### Metadata Storage (bincode)
- [x] Define `IndexedFontMeta` with serde derives (simplified from rkyv)
- [x] Implement bincode serialization/deserialization
- [x] Store font paths, names, classification data

### Path-to-ID Mapping
- [x] Implement `DB_PATH_TO_ID` for incremental updates
- [x] Store path hash → (FontID, mtime) mappings via xxhash
- [x] Use mtime comparison to skip unchanged files

### Ingestion Pipeline (`IndexWriter`)
- [x] Implement atomic write transactions
- [x] Support parallel font processing with rayon
- [x] Merge new entries with existing index (update-in-place)

### Query Execution (`IndexReader`)
- [x] Implement read transactions (non-blocking)
- [x] Query planner: tag bitmap intersection first
- [x] Apply numeric filters (weight/width/family-class)
- [x] Apply Roaring Bitmap cmap check for text queries
- [x] Apply name regex on metadata strings

### CLI Integration
- [x] Add `--index` flag to `cache add` (opt-in index mode)
- [x] Add `--index` flag to `cache find` (opt-in index mode)
- [x] Add `--index` flag to `cache list` (opt-in index mode)
- [x] Maintain JSON cache as default for backwards compatibility
- [x] Add `--index-path` flag for custom index location

### Testing
- [x] Unit tests for bitmap operations
- [x] Unit tests for cmap bitmap coverage
- [x] Unit tests for incremental update detection
- [x] Integration tests for full index round-trip (CLI)

### Future Work
- [x] Python bindings for indexed search (`find_indexed()`, `list_indexed()`, `count_indexed()`)
- [x] Cache clean --index support with `prune_missing()` for LMDB
- [x] Criterion benchmarks: live scan vs LMDB index query speed (benches/cache_vs_index.rs)
- [x] HTTP server `/search` index support (`use_index` and `index_path` fields)
- [ ] Benchmark: index build time on 10k/100k fonts (requires large font collection)

**Implementation Note:** Simplified from original plan by using bincode instead of rkyv (simpler API, sufficient performance) and Roaring Bitmap for cmap instead of Cuckoo Filter (deterministic, no false positives).
