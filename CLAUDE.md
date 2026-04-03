# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

made by FontLab https://www.fontlab.com/

## What is typg

Ultra-fast font search/discovery toolkit. Rust core with CLI and PyO3/Python bindings. Tracks fontgrep/fontgrepc semantics (live scan + JSON cache subcommands) while reusing fontations crates to stay lean.

## Workspace layout

Three-crate Cargo workspace (`Cargo.toml` at root with `resolver = "2"`):

| Crate | Path | Role |
|---|---|---|
| `typg-core` | `core/typg-core/` | Search engine: discovery, metadata extraction (via `read-fonts`/`skrifa`), query evaluation, output formatting |
| `typg-cli` | `cli/` | Clap-based CLI binary (`typg`): `find`, `cache {add,list,find,clean,info}`, `serve` subcommands |
| `typg-python` | `py/typg-python/` | PyO3 bindings + Fire CLI shim (`typgpy`). Published as the `typg` Python package |

Note: the workspace `Cargo.toml` references member names `typg-core`, `typg-cli`, `typg-python` — but `typg-core` lives at `core/typg-core/` (not at root), and `typg-python` lives at `py/typg-python/`.

### Other directories

- `linked/` — symlinks to sibling repos (`fontations`, `fontgrep`, `fontgrepc`, `fontlift`, `typf`) for reference. Read-only.
- `docs/spec.md` — CLI parity spec vs fontgrep/fontgrepc. Document any divergence here.
- `ARCHITECTURE.md` — data flow and design notes.

## Build commands

### Install

```bash
./install.sh                          # release install to ~/.cargo/bin
./install.sh --hpindex                # with LMDB index support
./install.sh --path /usr/local/bin    # custom install location
./install.sh --debug                  # debug build (faster compile)
```

### Rust (primary development loop)

```bash
# Full build script (release by default, emits Rust CLI + Python wheel)
./build.sh                    # release build
./build.sh debug              # debug build (faster compile)
./build.sh check              # cargo check + clippy + fmt, no binary
./build.sh clean              # clean target/

# Manual Rust commands
cargo build --workspace                           # debug build all crates
cargo build --workspace --release                  # release build
cargo build --workspace --features hpindex         # with LMDB index support
cargo fmt --check                                  # format check
cargo clippy --workspace -- -D warnings            # lint
cargo test --workspace                             # all tests
cargo test -p typg-core                            # core crate only
cargo test -p typg-cli                             # CLI integration tests only
cargo test -p typg-core --features hpindex         # index tests
```

### Python bindings

```bash
cd py/typg-python
uv venv --python 3.12
source .venv/bin/activate
uv pip install maturin
maturin develop --locked                           # dev install into venv
maturin develop --locked --features hpindex        # with index support

# Run Python tests
pytest tests/ -xvs

# Python CLI (after maturin develop)
typgpy find --paths ~/Fonts --scripts latn
```

Version is derived from git tags via `hatch-vcs` (not hardcoded in pyproject.toml).

### Publishing

```bash
./publish.sh check          # dry-run version sync check
./publish.sh publish        # full: sync versions from git tag → crates.io + PyPI
./publish.sh rust-only      # crates.io only
./publish.sh python-only    # PyPI only
```

CI: `.github/workflows/release.yml` triggers on `vN.N.N` tags. `.github/workflows/ci.yml` runs on PRs.

## Feature flags

The `hpindex` feature enables an LMDB-backed high-performance index (Roaring Bitmaps for O(K) tag intersection). It's optional and off by default.

- `typg-core`: `hpindex` adds deps: `heed`, `roaring`, `bytemuck`, `xxhash-rust`, `bincode`, `byteorder`. Enables `index` module.
- `typg-cli`: `hpindex` forwards to `typg-core/hpindex`. Enables `--index` flag on cache subcommands.
- `typg-python`: `hpindex` forwards to `typg-core/hpindex`. Exposes `find_indexed`, `list_indexed`, `count_indexed`.

Default feature: `fontations` (enables `read-fonts` + `skrifa`). Always on in practice.

## Architecture (data flow)

1. **Discovery** (`discovery.rs`): `PathDiscovery` walks directories via `walkdir`, finds font files (TTF/OTF/TTC/OTC/WOFF/WOFF2). Optionally follows symlinks, includes system font dirs.
2. **Metadata extraction** (`search.rs`): `read-fonts`/`skrifa` parse each face into `TypgFontFaceMeta` (names, axis/feature/script/table tags, codepoints, OS/2 fields). Parallel via `rayon`.
3. **Query evaluation** (`query.rs`): `Query` struct holds filter criteria (tags, name/creator/license regex, codepoints, weight/width ranges, family class, variable-only). `Query::matches(&meta)` returns bool.
4. **Output** (`output.rs`): `TypgFontFaceMatch` (meta + source path) streams to JSON/NDJSON/columns/plain text.
5. **Cache** (in `cli/src/lib.rs`): JSON file at `~/.cache/typg/cache.json` (or `TYPOG_CACHE_PATH`). Subcommands: `add` (ingest), `list`, `find` (query cached), `clean` (remove missing), `info`.
6. **Index** (`index.rs`, behind `hpindex`): LMDB at `~/.cache/typg/index/` (or `TYPOG_INDEX_PATH`). `FontIndex` struct with Roaring Bitmap inverted index for tags.
7. **HTTP server** (`server.rs`): Axum-based, `typg serve --bind addr`. Endpoints: `GET /health`, `POST /search`.

### Key types

- `Query` — filter specification (builder pattern: `.with_features()`, `.with_scripts()`, etc.)
- `TypgFontFaceMeta` — extracted metadata for one font face (serializable)
- `TypgFontFaceMatch` — metadata + file source info (the search result)
- `TypgFontSourceRef` — path to a discovered font file
- `SearchOptions` — parallelism config (`jobs`, `follow_symlinks`, `system_fonts`)
- `FontIndex` (hpindex) — LMDB index handle

### Python surface

Two packages exist in the wheel:
- `typg` (`python/typg/__init__.py`) — public API: re-exports `find`, `find_paths`, `filter_cached` (+ `find_indexed`, `list_indexed`, `count_indexed` if hpindex)
- `typg_python` (`python/typg_python/__init__.py`) — internal: imports from `_typg_python` native module, renames `find_py` → `find`, etc.

The native module is `typg_python._typg_python` (configured in `pyproject.toml` as `module-name`).

## Environment variables

| Variable | Purpose | Default |
|---|---|---|
| `TYPOG_SYSTEM_FONT_DIRS` | Override system font search paths (colon-separated) | OS defaults |
| `TYPOG_CACHE_PATH` | JSON cache file location | `~/.cache/typg/cache.json` |
| `TYPOG_INDEX_PATH` | LMDB index directory (hpindex) | `~/.cache/typg/index/` |

## Project conventions

- **fontgrep/fontgrepc parity**: CLI flags and output schemas should match fontgrep/fontgrepc. Document any divergence in `docs/spec.md`.
- **FontLab attribution**: files, CLI help, docs should carry `made by FontLab https://www.fontlab.com/` where appropriate.
- **Error handling**: `anyhow` in CLI/bindings, `thiserror` if typg-core needs typed errors. No `panic!` in library code.
- **Parallelism**: `rayon` for CPU work, `tokio` only in the HTTP server. `--jobs/-J` controls thread count.
- **Serialization**: `serde` + `serde_json` for all data types. Tags serialize as 4-char strings.
- **Confirm before deleting**: always ask user confirmation before running commands that delete files.
- **Testing**: property tests (`proptest`) for parsers in core, integration/snapshot tests for CLI, `pytest` for Python bindings. Criterion benchmarks behind `hpindex` feature.

## Version management

All three crates pin `version = "1.0.1"` and `typg-cli`/`typg-python` use `path` + exact version deps (`=1.0.1`) on `typg-core`. The `publish.sh` script syncs versions from the latest `vN.N.N` git tag before publishing.
