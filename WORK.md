# typg WORK log

made by FontLab https://www.fontlab.com/

| Date (UTC) | Goal | Notes | Tests |
| --- | --- | --- | --- |
| 2025-12-01 | PyO3 bindings + Fire CLI scaffold | Added `typg-python` crate with `find`/`filter_cached` bindings returning dicts, maturin `pyproject.toml`, and Fire-based `typgpy` wrapper; updated PLAN/TODO/README/CHANGELOG. | cargo test -p typg-python --tests; cargo test --workspace |
| 2025-12-01 | Bootstrap planning | Created PLAN/WORK/CHANGELOG, inventoried typf/fontgrep/fontgrepc/fontations/fontlift, defined success metrics and reuse choices | Not run (no test targets yet) |
| 2025-12-01 | Phase 1 spec + docs | Added docs/spec.md with fontgrep/fontgrepc parity matrix and crate layout, updated README.md and CLAUDE.md, checked off PLAN/TODO items | Not run (docs-only) |
| 2025-12-01 | Kick off typg-core | Documented no direct dependency on fontgrep/fontgrepc, scaffolded typg-core crate with filesystem discovery stub + tests; updated PLAN/TODO/spec | cargo fmt; cargo clippy -- -D warnings; cargo test |
| 2025-12-01 | Query parser + search pipeline | Added tag/codepoint parsers with property tests, baseline search pipeline with metadata extraction and NDJSON/JSON writers; updated PLAN/TODO | cargo fmt; cargo clippy -- -D warnings; cargo test |
| 2025-12-01 | Cache hook + CLI scaffold | Documented deferral of typf-fontdb cache integration, added cached metadata filtering in typg-core, scaffolded clap-based typg-cli find command with output modes | cargo fmt; cargo clippy -- -D warnings; cargo test |
| 2025-12-01 | CLI/discovery test hardening | Added nested/symlink discovery tests, NDJSON line-format check, and CLI help coverage; refreshed PLAN/TODO/CHANGELOG | cargo fmt; cargo clippy -- -D warnings; cargo test |
| 2025-12-01 | CLI stdin/system fonts/text filter | Added `--text` filter, STDIN path intake, and `--system-fonts` toggle with tests; updated PLAN/TODO/CHANGELOG | cargo fmt; cargo test |
| 2025-12-01 | CLI column output + bench | Added colorized/columnar text output with help snapshot tests; introduced criterion bench comparing codepoint parsers with fontgrep; updated PLAN/TODO/CHANGELOG | cargo fmt; cargo test |
