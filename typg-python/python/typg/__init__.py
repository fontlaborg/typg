"""
typg (made by FontLab https://www.fontlab.com/)

Lightweight shim so `pip install typg` exposes the same API as the
PyO3 bindings in `typg_python`.
"""

from __future__ import annotations

from importlib import metadata

from typg_python import filter_cached, find, find_paths

__all__ = ["find", "find_paths", "filter_cached", "__version__"]

# Optional indexed search functions (require hpindex feature in build)
try:
    from typg_python import count_indexed, find_indexed, list_indexed

    __all__.extend(["find_indexed", "list_indexed", "count_indexed"])
except ImportError:
    pass  # hpindex feature not enabled

try:
    __version__ = metadata.version(__name__)
except metadata.PackageNotFoundError:  # pragma: no cover - local editable installs
    __version__ = "0.0.0"
