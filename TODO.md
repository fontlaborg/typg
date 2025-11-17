# typg TODO

**Scope:** Build `typg`, a sister project to `typf`, focused on ultra-fast font search/discovery with a Rust library, CLI, and Python API. Requirements originate from `/Users/adam/Developer/vcs/TODO2.md` (Nov 16, 2025).

## Phase 0 – Groundwork
- [ ] Read `README.md`, `PLAN.md` (root), and `/Users/adam/Developer/vcs/TODO2.md` to internalize constraints
- [ ] Inventory relevant crates inside `../typf/` (fontdb, cache, metadata) for potential reuse instead of reimplementing logic
- [ ] Capture findings + tech choices in `PLAN.md` for typg (create if missing)
- [ ] Define success metrics (latency per query, supported filters, CLI parity with fontgrep/fontgrepc)

## Phase 1 – Source Analysis & Specification
- [ ] Deep-dive `external/fontgrep` (Rust) and `external/fontgrepc` (C) to catalog every flag, filter, and output format
- [ ] Decide which implementation elements to copy verbatim vs refactor (licensing + maintenance implications documented)
- [ ] Produce a comparison matrix (fontgrep/fontgrepc/typg) and store it in `docs/spec.md`
- [ ] Specify crate layout (e.g., `typg-core`, `typg-cli`, `typg-python`) and dependency relationships
- [ ] Document search use-cases (family name, axes presence, glyph coverage, Unicode range, weight/class filters)

## Phase 2 – Rust Library
- [ ] Scaffold `typg-core` crate with typf-compatible font discovery abstractions (build.rs to reuse fontdb indexes?)
- [ ] Implement parsers for CLI query syntax (borrow from fontgrep) with property tests
- [ ] Implement search pipeline: font loading → metadata extraction → filter evaluation → result ranking
- [ ] Integrate typf font cache APIs when advantageous; otherwise document why not
- [ ] Add streaming JSON/JSONL output plus structured Rust types
- [ ] Write criterion benchmarks comparing against original fontgrep; target parity before refactors

## Phase 3 – Rust CLI
- [ ] Build `typg-cli` crate with argument parser (clap/lexopt) mirroring fontgrep/fontgrepc options
- [ ] Ensure CLI supports recursive directory walks, system font discovery, and STDIN/STDOUT piping
- [ ] Implement colorized/columnar output plus `--json` / `--ndjson` toggles
- [ ] Add snapshot tests for CLI help + representative queries (use testdata from `external/fontgrep`)

## Phase 4 – Python Bindings & Fire CLI
- [ ] Design minimal PyO3 bindings that wrap `typg-core` search API (async-friendly if needed)
- [ ] Provide `fire`-based CLI mirroring Rust CLI semantics (optionally Typer if richer UX needed)
- [ ] Publish packaging metadata (pyproject via `maturin`) and document install flow in README
- [ ] Add pytest suite hitting bindings + CLIs with golden files shared with Rust tests

## Phase 5 – Documentation & Verification
- [ ] Update `README.md` with overview, install, usage examples (Rust, CLI, Python)
- [ ] Create `ARCHITECTURE.md` describing data flow + reuse points from typf
- [ ] Add CI workflow referencing typf + fontsimi pipelines
- [ ] Document migration guidance for existing fontgrep/fontgrepc users
- [ ] Record benchmarks + known limitations in `WORK.md` and `CHANGELOG.md`

## Phase 6 – Stretch
- [ ] Explore integration hooks so typg can directly feed fonts into typf/fontlift/testypf workflows
- [ ] Add optional gRPC/HTTP server mode for remote querying (only after core parity achieved)

**Testing Mandate:** Every new feature ships with unit tests + integration tests + benchmarks before marking TODO items complete.
