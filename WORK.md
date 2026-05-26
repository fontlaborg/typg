# typg WORK log

made by FontLab https://www.fontlab.com/

| Date (UTC) | Goal | Notes | Tests |
| --- | --- | --- | --- |
| 2026-05-26 | CLI Help Improvements | Added details keywords list directly in the CLI help message for `--details` / `-d` under `find`, `cache find`, and `cache list` commands. | cargo test (79 tests passed); cargo fmt --check; cargo clippy |
| 2026-05-26 | Details Keyword Renaming | Renamed 10 details keywords to shorter forms (var, wt, wd, panf, scr, scr_n, tab, tab_n, crea, lic) with backwards compatibility aliases. | cargo test --workspace --features hpindex (56 tests passed); uv run pytest (9 passed) |
| 2026-05-26 | Custom Details & CSV Output | Implemented name table detail extraction (psname, tfname, lfname, tsname, lsname), CLI --details / -d presets/custom filters, --csv output, and PyO3 Python binding parity. | cargo test --workspace --features hpindex (56 tests passed); uv run pytest (9 passed) |
| 2025-12-13 | Fix typg-python build | Repaired `convert_metadata()` definition (brace mismatch) and restored `#[pymodule]` so PyO3 module builds; updated typg-core doctest snippet to match current API. | cargo test --workspace; maturin pep517 build-wheel --compatibility off (typg-python) |
| 2025-12-03 | CLI polish: --quiet, cache info, --count | Added `--quiet` flag, `cache info` subcommand, `--count` flag for cache find. Updated README and CHANGELOG. 74 tests pass. | cargo test --workspace --features hpindex |
| 2025-12-03 | Phase 11 complete | hpindex feature fully implemented with LMDB, Roaring Bitmaps, bincode, criterion benchmarks, HTTP index support, Python bindings. | cargo test --workspace --features hpindex |
| 2025-12-01 | v1.0.0 release | Phases 1-6 complete: Rust library, CLI, Python bindings, HTTP server, CI/CD. See CHANGELOG.md for details. | cargo test --workspace |
