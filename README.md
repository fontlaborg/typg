# typg
made by FontLab https://www.fontlab.com/

Ultra-fast font search/discovery toolkit in Rust with a matching Python API. typg aims for flag-for-flag parity with `fontgrep` (live scan) and `fontgrepc` (cached), while reusing fontations/typf assets to stay lean.

## Status
Rust core and CLI are functional for live scans; cache subcommands and fuller Python test coverage are in progress. See `docs/spec.md` for the parity matrix and `ARCHITECTURE.md` for data flow.

## Install (from source today)
```bash
# Rust CLI
cargo install --path typg-cli

# Python bindings/CLI (uv-based)
uv venv --python 3.12
source .venv/bin/activate
uv pip install maturin
maturin develop --manifest-path typg-python/Cargo.toml --locked
```

## Quick usage
```bash
# Live scan for small caps + Latin coverage
typg find -f smcp -s latn ~/Fonts

# Pipe paths on stdin
fd .ttf ~/Fonts | typg find --stdin-paths --ndjson
```

### Python
```python
from typg_python import find

matches = find(paths=["~/Fonts"], scripts=["latn"], features=["smcp"])
for m in matches:
    print(m["path"], m["names"][0])
```

## Build & release
- macOS local builds: `./build.sh [release|debug]` emits the `typg` Rust CLI and the `typg` Python wheel/`typgpy` CLI (version comes from git tags via hatch-vcs).
- Manual publishing: `./publish.sh [publish|rust-only|python-only|sync|check]` syncs Cargo crate versions to the current semver git tag, then pushes crates to crates.io and wheels to PyPI when credentials are present.
- GitHub Actions: `release.yml` triggers on `vN.N.N` tags to build manylinux/macOS/Windows wheels, publish to PyPI, publish crates (`typg-core`, `typg-cli`, `typg-python`) to crates.io, and attach wheels to the GitHub release.

## Migration notes (fontgrep/fontgrepc → typg)
- Flags mirror fontgrep; see `docs/spec.md` for any divergence.
- Cache subcommands are being built; until then typg only performs live scans.
- NDJSON output matches fontgrepc conventions so log pipelines stay compatible.

## Contributing
- Keep functions short and prefer deleting over adding.
- Match fontgrep/fontgrepc semantics unless a deviation is documented in `docs/spec.md`.
- Add tests (property for parsers, snapshot for CLI) before marking tasks done.

## Project links
- Plan: `PLAN.md`
- Tasks: `TODO.md`
- Spec: `docs/spec.md`
- Architecture: `ARCHITECTURE.md`
- Work log: `WORK.md`
