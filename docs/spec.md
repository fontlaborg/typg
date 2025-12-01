# typg Specification (draft)
made by FontLab https://www.fontlab.com/

## Purpose
Map typg’s planned behavior against the two reference CLIs (fontgrep, fontgrepc) and pin down the initial crate layout plus core search use-cases.

## Comparison matrix
| Capability | fontgrep (live scan) | fontgrepc (cached) | typg (planned) |
| --- | --- | --- | --- |
| Execution model | Walks provided paths recursively and inspects fonts on the fly | SQLite cache with `add/list/clean/find` subcommands | Dual-path: live scan for ad‑hoc runs; optional cache module modeled after fontgrepc |
| Core query filters | `--axes/-a`, `--features/-f`, `--scripts/-s`, `--tables/-T`, `--variable/-v`, `--name/-n` (regex), `--codepoints/-u`, `--text/-t`, `--jobs/-J` | Same filter set on `find` plus path filtering; `--jobs/-j` on `add`, `--force` | Match full filter set; add weight/class shorthands and Unicode range presets; keep regex for names |
| Output modes | Plain text (paths), optional `--json/-j`; progressive printing | Plain text or `--json`; verbose summary when `--verbose` | Structured Rust types → text/JSON/NDJSON; configurable columns; `--quiet`/`--stats` toggles |
| Parallelism | `-J/--jobs` for search | `-j/--jobs` for cache ingest; rayon pool | Thread-pool for ingest; async façade for Python bindings; sane defaults |
| Cache control | None | `--cache-path`, `clean`, `list` | Cache optional: default path + `--cache-path`; background revalidate hook |
| CLI surface | Single command | Subcommands: `find`, `add`, `list`, `clean` | `typg` mirrors subcommands when cache enabled; `typg find` matches fontgrep flags |
| Dependencies | skrifa/read-fonts, jwalk, clap | clap, rusqlite, rayon, jwalk | Prefer fontations (`read-fonts`/`skrifa`) for parsing; reuse typf-fontdb when it stays lean |

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
- **Classification filters**: weight/class/width shorthand mapped onto OS/2 values when present.
- **Performance knobs**: control job count, cache usage, and output verbosity without changing query semantics.

## Parity targets (Phase 1)
- Match every fontgrep flag and fontgrepc subcommand semantics where they overlap.
- Keep dependency tree slimmer than fontgrepc (prefer existing fontations + typf assets over new crates).
- Provide JSON/NDJSON outputs identical between CLI and Python bindings.
- Dependency stance: copy/port logic from fontgrep/fontgrepc when needed but avoid taking those crates as direct dependencies; rely instead on their shared dependencies (e.g., clap, rayon, fontations).
