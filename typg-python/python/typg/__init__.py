"""
typg (made by FontLab https://www.fontlab.com/)

Lightweight shim so `pip install typg` exposes the same API as the
PyO3 bindings in `typg_python`.
"""

from __future__ import annotations

from importlib import metadata

from typg_python import filter_cached, find

__all__ = ["find", "filter_cached", "__version__"]

try:
    __version__ = metadata.version(__name__)
except metadata.PackageNotFoundError:  # pragma: no cover - local editable installs
    __version__ = "0.0.0"
