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
- [~] Build `typg-cli` crate with argument parser (clap/lexopt) mirroring fontgrep/fontgrepc options
- [x] `typg-cli` find subcommand scaffolded (axes/features/scripts/tables/name/codepoints/variable flags + json/ndjson/plain output)
- [x] Ensure CLI supports recursive directory walks, system font discovery, and STDIN/STDOUT piping
- [x] Implement colorized/columnar output plus `--json` / `--ndjson` toggles
- [~] Add snapshot tests for CLI help (done) plus representative queries (pending font fixtures from `external/fontgrep`)

## Phase 4 – Python Bindings & Fire CLI
- [x] Design minimal PyO3 bindings that wrap `typg-core` search API (async-friendly if needed)
- [x] Provide `fire`-based CLI mirroring Rust CLI semantics (optionally Typer if richer UX needed)
- [x] Publish packaging metadata (pyproject via `maturin`) and document install flow in README
- [~] Add pytest suite hitting bindings + CLIs with golden files shared with Rust tests

## Phase 5 – Documentation & Verification
- [ ] Update `README.md` with overview, install, usage examples (Rust, CLI, Python)
- [ ] Create `ARCHITECTURE.md` describing data flow + reuse points from typf
- [ ] Add CI workflow referencing typf + fontsimi pipelines
- [x] Add release workflow to publish crates (typg-core/typg-cli/typg-python) and PyPI wheels on semver tags
- [ ] Document migration guidance for existing fontgrep/fontgrepc users
- [ ] Record benchmarks + known limitations in `WORK.md` and `CHANGELOG.md`

## Phase 6 – Stretch
- [ ] Explore integration hooks so typg can directly feed fonts into typf/fontlift/testypf workflows
- [ ] Add optional gRPC/HTTP server mode for remote querying (only after core parity achieved)

**Testing Mandate:** Every new feature ships with unit tests + integration tests + benchmarks before marking TODO items complete.
