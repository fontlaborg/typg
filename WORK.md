# typg WORK log

made by FontLab https://www.fontlab.com/

| Date (UTC) | Goal | Notes | Tests |
| --- | --- | --- | --- |
| 2025-12-13 | Fix typg-python build | Repaired `convert_metadata()` definition (brace mismatch) and restored `#[pymodule]` so PyO3 module builds; updated typg-core doctest snippet to match current API. | cargo test --workspace; maturin pep517 build-wheel --compatibility off (typg-python) |
| 2025-12-03 | CLI polish: --quiet, cache info, --count | Added `--quiet` flag, `cache info` subcommand, `--count` flag for cache find. Updated README and CHANGELOG. 74 tests pass. | cargo test --workspace --features hpindex |
| 2025-12-03 | Phase 11 complete | hpindex feature fully implemented with LMDB, Roaring Bitmaps, bincode, criterion benchmarks, HTTP index support, Python bindings. | cargo test --workspace --features hpindex |
| 2025-12-01 | v1.0.0 release | Phases 1-6 complete: Rust library, CLI, Python bindings, HTTP server, CI/CD. See CHANGELOG.md for details. | cargo test --workspace |
