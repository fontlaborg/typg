"""
Font discovery that feels like finding your favorite coffee shop.

These Python wrappers expose typg's Rust-powered font search with the gentle 
warmth you expect from Python. Like having a friend who speaks both languages 
fluently - translating between Rust's speed and Python's comfort without breaking 
a sweat. No caffeine required, but fonts may become surprisingly addictive.
"""

# Core search functions - always ready for action
from ._typg_python import filter_cached_py as filter_cached
from ._typg_python import find_py as find
from ._typg_python import find_paths_py as find_paths

__all__ = ["find", "find_paths", "filter_cached"]

# Premium indexed search - like having a personal font librarian
# Only appears if you built with the hpindex feature flag
try:
    from ._typg_python import count_indexed_py as count_indexed
    from ._typg_python import find_indexed_py as find_indexed
    from ._typg_python import list_indexed_py as list_indexed

    __all__.extend(["find_indexed", "list_indexed", "count_indexed"])
except ImportError:
    pass  # Feature flag not enabled - enjoy the standard experience
