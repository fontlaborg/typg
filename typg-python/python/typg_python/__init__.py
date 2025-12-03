"""
Python API for typg.

Exports thin wrappers around the Rust bindings defined in `_typg_python`.
"""

from ._typg_python import filter_cached_py as filter_cached
from ._typg_python import find_py as find
from ._typg_python import find_paths_py as find_paths

__all__ = ["find", "find_paths", "filter_cached"]

# Optional indexed search functions (require hpindex feature in build)
try:
    from ._typg_python import count_indexed_py as count_indexed
    from ._typg_python import find_indexed_py as find_indexed
    from ._typg_python import list_indexed_py as list_indexed

    __all__.extend(["find_indexed", "list_indexed", "count_indexed"])
except ImportError:
    pass  # hpindex feature not enabled
