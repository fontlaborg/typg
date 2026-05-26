# typg - Custom Font Details & CSV Output Plan
made by FontLab https://www.fontlab.com/

This document outlines the detailed implementation plan for adding custom detail selection (`--details` / `-d`) and a new CSV output format (`--csv`) to the `typg find` and cached/indexed search surfaces.

---

## 1. Goal Description
Extend `typg find` to support fine-grained control over the metadata attributes included in search results via `--details` / `-d`, supporting both predefined integer presets (e.g., `0`, `1`, `2`, `5`, `8`, `9`) and explicit comma-separated lists of property keywords. The selected details will be rendered in the requested format: JSON, NDJSON, padded columns, or a new CSV output format (`--csv`).

---

## 2. Checklist

### Phase 1 — Core Metadata Enhancements (typg-core)
- [x] **Extend `TypgFontFaceMeta`** in [search.rs](file:///Users/adam/Developer/vcs3/github.fontlaborg/typg/core/typg-core/src/search.rs):
  - Add optional name table entries:
    - `psname: Option<String>` (PostScript Name, ID 6)
    - `tfname: Option<String>` (Typographic Family Name, ID 16)
    - `lfname: Option<String>` (Legacy/Font Family Name, ID 1)
    - `tsname: Option<String>` (Typographic Subfamily/Style Name, ID 17)
    - `lsname: Option<String>` (Legacy/Font Subfamily/Style Name, ID 2)
  - Ensure compatibility by adding `#[serde(default)]` to these fields to allow deserializing legacy JSON caches.
- [x] **Optimize Name Table Extraction** in [search.rs](file:///Users/adam/Developer/vcs3/github.fontlaborg/typg/core/typg-core/src/search.rs):
  - Refactor or create a helper `collect_names_and_details` that traverses the `name` table records in a single pass to collect all relevant name IDs (1, 2, 4, 6, 16, 17) in Unicode encodings.
  - Update `collect_names` to populate both the global `names` fallback list and the specific optional fields.
- [x] **Extend `IndexedFontMeta`** in [index.rs](file:///Users/adam/Developer/vcs3/github.fontlaborg/typg/core/typg-core/src/index.rs):
  - Add `psname`, `tfname`, `lfname`, `tsname`, `lsname` fields with `#[serde(default)]`.
- [x] **Refactor `FontIndex::writer::add_font`** in [index.rs](file:///Users/adam/Developer/vcs3/github.fontlaborg/typg/core/typg-core/src/index.rs):
  - Change signature from accepting a massive list of parameters (14 fields) to accepting:
    - `path: &Path`
    - `ttc_index: Option<u32>`
    - `mtime: SystemTime`
    - `meta: &TypgFontFaceMeta`
  - Update `hydrate_match` to copy the newly indexed specific name fields from `IndexedFontMeta` to `TypgFontFaceMeta`.
- [x] **Update `add_font` Call Sites**:
  - Update tests in [index.rs](file:///Users/adam/Developer/vcs3/github.fontlaborg/typg/core/typg-core/src/index.rs) to construct and pass `TypgFontFaceMeta`.
  - Update criterion benchmarks in [cache_vs_index.rs](file:///Users/adam/Developer/vcs3/github.fontlaborg/typg/core/typg-core/benches/cache_vs_index.rs).
  - Update `cli/src/lib.rs` and `cli/src/server.rs`.
  - Update PyO3 binding tests in `py/typg-python/src/lib.rs`.

### Phase 2 — CLI Struct Changes (typg-cli)
- [x] **Extend `OutputArgs` and `FindArgs`** in [cli/src/lib.rs](file:///Users/adam/Developer/vcs3/github.fontlaborg/typg/cli/src/lib.rs):
  - Add `--csv` flag, mutually exclusive with `--json`, `--ndjson`, `--paths`, `--columns`, and `--count`.
  - Add `--details / -d <DETAILS>` option taking a string.
- [x] **Extend `OutputFormat`** in [cli/src/lib.rs](file:///Users/adam/Developer/vcs3/github.fontlaborg/typg/cli/src/lib.rs):
  - Add `csv: bool` and `details: Option<String>` to `OutputFormat`.
  - Parse the `--details` / `-d` argument. Support preset parsing:
    - `0` -> `path` (only paths, equivalent to `--paths` but in the selected format)
    - `1` -> `path,fname,sname,fmt`
    - `2` -> `path,fname,sname,fmt,psname,is_var`
    - `5` -> `path,fname,sname,fmt,psname,is_var,weight,width,family_class`
    - `8` -> `path,fname,sname,fmt,psname,is_var,weight,width,family_class,axes,fea_n,scripts`
    - `9` -> all keywords
  - If a comma-separated list is provided, validate each property string:
    - Keywords: `path`, `path_r`, `fmt`, `fname`, `sname`, `psname`, `tfname`, `lfname`, `tsname`, `lsname`, `is_var` (or `variable`), `weight`, `width`, `family_class`, `axes`, `axes_n`, `fea`, `fea_n`, `scripts`, `scripts_n`, `tables`, `tables_n`, `creator_names`, `license_names`.

### Phase 3 — Details Filtering & Formatting
- [x] **Implement Relative Path Solver**:
  - Implement a helper to find the relative path of a font file to the corresponding search root under which it was discovered.
- [x] **Implement Value Extraction**:
  - Write a helper function that takes `&TypgFontFaceMatch`, the current search roots, and a property keyword, and returns a formatted JSON value (`serde_json::Value`).
    - Handle conversion (e.g. lists into arrays/comma-separated strings, numbers to ints, booleans to bools).
- [x] **Update Output Formats**:
  - **JSON**: If `--details` is specified, filter each match into a map containing only requested keys. If `-d 0`, render as a simple array of strings.
  - **NDJSON**: Same as JSON, but serialized as one line per match.
  - **CSV**: Implement CSV header printing and row writing. Quote and escape fields containing commas, double-quotes, or newlines. Allow streaming row by row.
  - **Columns**: Render the selected properties as padded columns aligned according to their maximum value length.
- [x] **Support Cache Commands Integration**:
  - Ensure that `cache list` and `cache find` also respect `--details` and `--csv` through the flattened `OutputArgs`.

### Phase 4 — Python Bindings (typg-python)
- [x] **Extend Python Bindings** in [py/typg-python/src/lib.rs](file:///Users/adam/Developer/vcs3/github.fontlaborg/typg/py/typg-python/src/lib.rs):
  - Add specific name fields to `MetadataInput`.
  - Expose `psname`, `tfname`, `lfname`, `tsname`, `lsname` in the returned Python dictionary in `to_py_matches`.
- [x] **Verify Python Bindings**:
  - Run python tests to ensure changes to `TypgFontFaceMeta` compile and load correctly.

### Phase 5 — Verification & Tests
- [x] **Write Unit Tests**:
  - Verify name extraction parses unicode name records correctly.
  - Verify detail property extraction matches correct names and classes.
  - Verify relative path solver works with multiple search roots and fallbacks.
- [x] **Write CLI Tests**:
  - Test preset inputs (`-d 0`, `-d 1`, etc.).
  - Test custom lists (e.g. `-d path,fname,fmt`).
  - Test CSV output format, ensuring proper escaping.
  - Test compatibility with the LMDB index feature.
- [x] **Verify End-To-End Performance**:
  - Ensure that custom details and CSV serialization do not introduce latency regressions on large datasets.
