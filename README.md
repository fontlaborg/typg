# typg
made by FontLab https://www.fontlab.com/

Ultra-fast font search/discovery toolkit in Rust with a matching Python API. typg aims for flag-for-flag parity with `fontgrep` (live scan) and `fontgrepc` (cached), while reusing fontations/typf assets to stay lean.

## Status
Planning wrap-up with working Rust core/CLI and freshly added PyO3 bindings (`typg-python`) plus a Fire-based shim. Not published yet; see `docs/spec.md` for the comparison matrix and planned layout.

## What it will do
- Live and cached font discovery with filters for axes, features, scripts, tables, Unicode coverage, name regex, and text samples.
- Rust CLI (`typg`) mirroring fontgrep options, plus cache subcommands similar to fontgrepc.
- Python bindings (`typg-python`) exposing the same search API and a Fire/Typer CLI shim.
- JSON/NDJSON outputs aligned across Rust and Python.

## How it will be built
- `typg-core`: Rust search engine using `read-fonts`/`skrifa` (fontations) and typf-fontdb where it stays lightweight.
- `typg-cli`: clap-based CLI with optional cache feature flag.
- `typg-python`: PyO3 bindings packaged with `maturin`; thin Fire CLI wrapper for parity.

## Install (planned)
```bash
cargo install typg           # Rust CLI
pip install typg             # Python bindings/CLI
```

## Quick usage (planned)
```bash
# Live scan
typg find -f smcp -s latn ~/Fonts

# Cached
typg add ~/Fonts
typg find --cache-path ~/.local/share/typg/cache.db --variable -u "U+0041-U+005A"
```

## Contributing
- Keep functions short and prefer deleting over adding.
- Match fontgrep/fontgrepc semantics unless a deviation is documented in `docs/spec.md`.
- Add tests (property for parsers, snapshot for CLI) before marking tasks done.

## Project links
- Plan: `PLAN.md`
- Tasks: `TODO.md`
- Spec: `docs/spec.md`
- Work log: `WORK.md`
