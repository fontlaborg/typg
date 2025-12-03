# typg
made by FontLab https://www.fontlab.com/

Ultra-fast font search/discovery toolkit in Rust with a matching Python API. typg tracks fontgrep/fontgrepc semantics (live scan today; cache subcommands next) while reusing fontations + typf assets to stay lean.

## Components
- `typg-core`: search engine built on `read-fonts`/`skrifa` (fontations) with cached-filter hooks.
- `typg-cli`: clap-based CLI that mirrors fontgrep’s `find` flags and output modes (plain/columns/JSON/NDJSON).
- `typg-python`: PyO3 bindings plus a Fire/Typer CLI shim (`typgpy`) so Python users get the same surface.

## Status
- Live scans work: axes/features/scripts/tables/name/regex/codepoints/text filters plus STDIN/system font discovery.
- Not yet: cache subcommands (`add/list/clean/find`), job controls, weight/class shorthands. These remain parity gaps to close.
- Docs/spec cover planned parity (`docs/spec.md`); architecture notes live in `ARCHITECTURE.md`.

## Install (source, today)
```bash
# Rust CLI
cargo install --path typg-cli

# Python bindings/CLI (uv-based)
uv venv --python 3.12
source .venv/bin/activate
uv pip install maturin
maturin develop --manifest-path typg-python/Cargo.toml --locked
```

## Usage
### CLI (`typg`)
- Live scan a directory for small caps + Latin support: `typg find -f smcp -s latn ~/Fonts`
- Accept STDIN paths: `fd .ttf ~/Fonts | typg find --stdin-paths --ndjson`
- Include system font roots: `typg find --system-fonts --columns`
- JSON output: add `--json` (array) or `--ndjson` (one match per line). Columns/plain auto-colorize unless `--color never`.
- Path overrides for system fonts: set `TYPOG_SYSTEM_FONT_DIRS="/opt/fonts:/tmp/fonts"`.

### Python (`typg` / `typgpy`)
```python
from typg import find

matches = find(paths=["~/Fonts"], scripts=["latn"], features=["smcp"], variable=True)
for m in matches:
    print(m["path"], m["names"][0])
```

CLI parity from Python: `typgpy find --paths ~/Fonts --scripts latn --features smcp --variable`.

### Rust library (`typg-core`)
```rust
use std::path::PathBuf;
use typg_core::query::Query;
use typg_core::search::{search, SearchOptions};
use typg_core::tags::tag4;

let paths = vec![PathBuf::from("~/Fonts")];
let query = Query::new().with_features(vec![tag4("smcp").unwrap()]);
let matches = search(&paths, &query, &SearchOptions::default())?;
```

## Migration (fontgrep/fontgrepc)
- `typg find` mirrors `fontgrep find` flags already shipped (axes/features/scripts/tables/name/regex/codepoints/text, STDIN, system fonts, JSON/NDJSON, columns/plain).
- Missing today: cache subcommands (`add/list/clean/find`) and `--jobs` parallelism—keep using fontgrepc for cached bulk searches until typg’s cache lands.
- Weight/class/width shorthands are still planned; use explicit tag filters for now.
- Output layout matches fontgrepc NDJSON; column widths are stable for downstream tooling.

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
