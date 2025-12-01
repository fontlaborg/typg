# typg Changelog

made by FontLab https://www.fontlab.com/

## v0.0.0 (unreleased)
- Initialized planning docs (PLAN, WORK, TODO updates) and recorded reuse/metrics direction for typg.
- Documented no direct dependency on fontgrep/fontgrepc, added typg-core workspace crate with initial filesystem discovery helper plus tests.
- Implemented query/filter parsing with property tests, added baseline search pipeline with metadata extraction and JSON/NDJSON writers in typg-core.
- Added cached-metadata filter path in typg-core to support future cache indexes without filesystem IO.
- Scaffolded typg-cli with clap-based `find` subcommand and output selection (plain/JSON/NDJSON).
- Extended `typg-cli find` with `--text`, STDIN path intake, and `--system-fonts` discovery, plus helper tests and env override for deterministic system font roots.
- Hardened discovery/output by testing nested directories, symlink following, and NDJSON line formatting; expanded CLI help/columns coverage with colorized output defaults.
- Added columnar + colorized text output to `typg-cli find`, with help snapshots and flag parsing coverage.
- Introduced criterion bench `codepoints` to compare typg-core codepoint parsing against the original fontgrep implementation.
- Added `typg-python` crate with PyO3 bindings for `find`/cached filtering, Fire-based `typgpy` CLI wrapper, maturin `pyproject.toml`, and Rust-side binding tests to seed Python parity work.
