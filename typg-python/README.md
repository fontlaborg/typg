# typg (PyPI)

PyO3 bindings for `typg-core` (made by FontLab https://www.fontlab.com/), exposed on PyPI as `typg` with a Fire-based CLI wrapper.

## Build

```bash
uv venv --python 3.12
uv pip install maturin
uv run maturin develop --features extension-module
```

## Usage

```python
from typg import find, filter_cached

results = find([\"/Library/Fonts\"], axes=[\"wght\"], variable=True, jobs=4)
print(len(results))

weighted = find([\"/Library/Fonts\"], weight=\"300-500\", width=\"5\")
print(\"weight/width matches\", len(weighted))
```

CLI:

```bash
typgpy find --paths /Library/Fonts --axes wght --weight 300-500 --variable --jobs 4
```
