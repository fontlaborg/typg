# typg Architecture
made by FontLab https://www.fontlab.com/

## Overview
typg is a three-crate workspace:
- `typg-core`: discovers font files, extracts metadata with `read-fonts`/`skrifa`, and evaluates queries.
- `typg-cli`: clap front-end that maps fontgrep/fontgrepc flags onto `typg-core`, formats output (plain/columns/JSON/NDJSON), and will house cache subcommands.
- `typg-python`: PyO3 bindings exposing the same search primitives with a thin Fire/Typer CLI shim.

## Data flow (live scan)
1. **Discovery**: `PathDiscovery` walks provided roots (optionally follows symlinks or system font dirs).
2. **Metadata load**: `read-fonts` parses each face (handles TTC/OTC) into names, tables, feature/script tags, axes, and codepoints via `skrifa`.
3. **Filtering**: `Query` matches tags, regexes, codepoints, and variable-ness; cached metadata can skip file IO via `filter_cached`.
4. **Output**: `TypgFontFaceMatch` structs stream to JSON/NDJSON or columnar/plain text; Python bindings return dicts.

## Reuse points from typf/fontations
- Font parsing relies on `read-fonts`/`skrifa` (fontations) for zero-copy table access.
- Cache design will mirror `typf-fontdb` and fontgrepc schema while keeping dependency footprint minimal.
- System font roots align with `fontlift` defaults; environment overrides (`TYPOG_SYSTEM_FONT_DIRS`) allow platform-safe testing.

## Pending extensions
- Cache module: SQLite-backed ingest/list/find/clean patterned after fontgrepc and typf-fontdb.
- Job control: opt-in parallel discovery/IO using Rayon once cache hooks land, with CLI `--jobs` flag.
- Server mode (stretch): optional gRPC/HTTP façade once CLI parity is solid.

## Testing strategy
- Property tests for tag/codepoint parsers in `typg-core`.
- Snapshot tests for CLI help/output, shared golden files with Python bindings.
- Criterion benches (e.g., codepoint parsing) tracked in `WORK.md` alongside known limitations.
