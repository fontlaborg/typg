# typg Architecture
made by FontLab https://www.fontlab.com/

## Overview
typg is a three-crate workspace that keeps parsing/rendering logic in the Rust core and mirrors the same surface across CLIs:
- `typg-core`: discovers font files, extracts metadata with `read-fonts`/`skrifa`, and evaluates queries.
- `typg-cli`: clap front-end that maps fontgrep/fontgrepc flags onto `typg-core`, formats output (plain/columns/JSON/NDJSON), and will house cache subcommands.
- `typg-python`: PyO3 bindings exposing the same search primitives with a thin Fire/Typer CLI shim.

## Data flow (live scan)
1. **Discovery**: `PathDiscovery` walks provided roots (optionally follows symlinks or system font dirs via `TYPOG_SYSTEM_FONT_DIRS`).
2. **Metadata load**: `read-fonts` parses each face (handles TTC/OTC) into names, tables, feature/script tags, axes, and codepoints via `skrifa`.
3. **Filtering**: `Query` matches tags, regexes, codepoints, and variable-ness; cached metadata can skip file IO via `filter_cached`.
4. **Output**: `TypgFontFaceMatch` structs stream to JSON/NDJSON or columnar/plain text; Python bindings return dicts.

## Reuse points from typf/fontations
- Font parsing relies on `read-fonts`/`skrifa` (fontations) for zero-copy table access.
- Cache design will mirror `typf-fontdb` and fontgrepc schema while keeping dependency footprint minimal.
- System font roots align with `fontlift` defaults; environment overrides (`TYPOG_SYSTEM_FONT_DIRS`) allow platform-safe testing.
- Test fixtures come from `typf/test-fonts` so CLI/python parity tests share goldens without bloating this repo.

## Cache/parallel path
- **Cache ingest/list/find/clean**: mirror fontgrepc UX via a JSON cache file today; future work may swap to a SQLite/typf-fontdb-backed index.
- **Job control**: rayon-backed parallel discovery and IO exposed via `--jobs/-J` for live scans and cache ingest.
- **Python parity**: expose cache APIs through PyO3 so `typgpy` can drive both live scans and cached queries.

## Current limitations
- Cache uses a JSON file without automatic revalidation; SQLite/typf-fontdb integration is still planned.
- Weight/class/width shorthands are not mapped; rely on explicit feature/script/table/tag filters.
- Python bindings only exercise Rust-side tests today; full pytest coverage is pending.

## Testing strategy
- Property tests for tag/codepoint parsers in `typg-core`.
- Snapshot tests for CLI help/output, shared golden files with Python bindings.
- Criterion benches (e.g., codepoint parsing) tracked in `WORK.md` alongside known limitations.
