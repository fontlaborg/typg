# typg Specification (draft)
made by FontLab https://www.fontlab.com/

## Purpose
Map typg’s planned behavior against the two reference CLIs (fontgrep, fontgrepc) and pin down the initial crate layout plus core search use-cases.

## Comparison matrix
| Capability | fontgrep (live scan) | fontgrepc (cached) | typg (current) |
| --- | --- | --- | --- |
| Execution model | Walks provided paths recursively and inspects fonts on the fly | SQLite cache with `add/list/clean/find` subcommands | Tri-path: live scan for ad‑hoc runs; JSON cache subcommands; **LMDB index** (`--index` flag, `hpindex` feature) for O(K) queries on 100k+ fonts |
| Core query filters | `--axes/-a`, `--features/-f`, `--scripts/-s`, `--tables/-T`, `--variable/-v`, `--name/-n` (regex), `--codepoints/-u`, `--text/-t`, `--jobs/-J` | Same filter set on `find` plus path filtering; `--jobs/-j` on `add`, `--force` | Match full filter set; OS/2 weight/width/family-class shorthands shipped; keep regex for names |
| Output modes | Plain text (paths), optional `--json/-j`; progressive printing | Plain text or `--json`; verbose summary when `--verbose` | Structured Rust types → text/JSON/NDJSON; configurable columns; `--paths` for piping |
| Parallelism | `-J/--jobs` for search | `-j/--jobs` for cache ingest; rayon pool | Thread-pool for ingest; async façade for Python bindings; sane defaults |
| Cache control | None | `--cache-path`, `clean`, `list` | JSON cache + optional LMDB index (`--index-path`); `clean` prunes missing entries |
| CLI surface | Single command | Subcommands: `find`, `add`, `list`, `clean` | `typg find` matches fontgrep flags; `typg cache add/list/find/clean` manage cache/index; `typg serve` for HTTP |
| Dependencies | skrifa/read-fonts, jwalk, clap | clap, rusqlite, rayon, jwalk | fontations (`read-fonts`/`skrifa`); heed/roaring for index (optional `hpindex` feature) |

## Crate layout (initial)
- `typg-core`: search engine; font parsing via `read-fonts`/`skrifa` (fontations) and/or typf-fontdb index adapter; filter evaluation and result structs.
- `typg-cli`: command-line wrapper mirroring fontgrep/fontgrepc flags; optional cache subcommands gated behind feature flag.
- `typg-python`: PyO3 bindings exposing core search API plus Fire/Typer CLI shim to maintain parity with Rust CLI.

## Core search use-cases
- **Family/name lookup**: regex/substring matches against full name and postscript name.
- **Variation axes presence**: ensure fonts define requested axis tags (wght, wdth, ital, opsz, etc.).
- **OpenType features/scripts**: filter by GSUB/GPOS feature tags and script systems present.
- **Unicode coverage**: accept codepoints, ranges, or text samples; return only fonts covering all requested characters.
- **Table presence**: require tables like GPOS/GSUB/GDEF/BASE/OS/2.
- **Classification filters**: OS/2 weight/width/family-class shorthands mapped onto `usWeightClass`/`usWidthClass` and `sFamilyClass` (major or `major.subclass`, with name aliases like `sans`, `ornamental`, `script`).
- **Performance knobs**: control job count, cache usage, and output verbosity without changing query semantics.

## High-Performance Index (`hpindex` feature)

For 100k+ font collections, typg offers an optional LMDB-backed index that delivers O(K) query performance:

**Architecture:**
- **Storage**: LMDB via `heed` crate for memory-mapped, zero-copy reads
- **Tag queries**: Roaring Bitmaps for ultra-fast set intersections
- **Serialization**: bincode for font metadata
- **Incremental updates**: xxhash path hashing + mtime comparison

**CLI usage:**
```bash
# Build index
typg cache add --index ~/Fonts

# Query index (O(K) vs O(N) live scan)
typg cache find --index --scripts latn --features smcp

# Custom location
typg cache add --index --index-path /path/to/index ~/Fonts
```

**Python bindings:**
```python
from typg import find_indexed, list_indexed, count_indexed
matches = find_indexed(index_path="~/.cache/typg/index", scripts=["latn"])
```

**HTTP server:**
```json
POST /search
{"use_index": true, "index_path": "/path/to/index", "scripts": ["latn"]}
```

## Parity targets (Phase 1)
- Match every fontgrep flag and fontgrepc subcommand semantics where they overlap.
- Keep dependency tree slimmer than fontgrepc (prefer existing fontations + typf assets over new crates).
- Provide JSON/NDJSON outputs identical between CLI and Python bindings.
- Dependency stance: copy/port logic from fontgrep/fontgrepc when needed but avoid taking those crates as direct dependencies; rely instead on their shared dependencies (e.g., clap, rayon, fontations).
