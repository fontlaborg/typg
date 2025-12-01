# typg-python

PyO3 bindings for `typg-core`, exposing the same search filters from Rust to Python plus a Fire-based CLI wrapper.

## Build

```bash
uv venv --python 3.12
uv pip install maturin
uv run maturin develop --features extension-module
```

## Usage

```python
from typg_python import find, filter_cached

results = find([\"/Library/Fonts\"], axes=[\"wght\"], variable=True, json=False)
print(len(results))
```

CLI:

```bash
typgpy find --paths /Library/Fonts --axes wght --variable
```
