"""
typg - Your friendly neighborhood font wrangler

Like a coffee-fueled librarian who secretly remembers every book's favorite page,
typg wanders through font directories and hands you exactly what you wanted.
No more clicking through endless folders - just ask and watch the magic happen.

Built with care by FontLab https://www.fontlab.com/ - because finding fonts
should feel like running into an old friend, not searching for lost keys.

## The Essentials

- **find()**: Stroll through directories, collect font friends
- **find_paths()**: Just the GPS coordinates when you want to visit yourself  
- **filter_cached()**: Browse your collection without bothering the disk
- **find_indexed()**: Sprint through pre-built indexes when caffeine wears off

## How it Rolling

```python
import typg

# Find variable fonts that understand Arabic perfectly
results = typg.find(
    paths=["/System/Library/Fonts", "~/fonts"],
    query=lambda q: q.has_script("Arab").has_axis("wght").is_variable()
)

# Discover fonts with familiar faces
sans_fonts = typg.find(
    paths=["/usr/share/fonts"],
    name_pattern=r".*Arial.*"
)

# Browse cached data without waking up the file system
installed_fonts = typg.filter_cached(cached_data, query=lambda q: q.has_feature("liga"))
```

## Speak Your Language

The query system actually listens:
- Script support (has_script) - "Find fonts that read Arabic"
- Variable font axes (has_axis) - "Show me fonts with weight knobs"  
- OpenType features (has_feature) - "I need those pretty ligatures"
- Font names (name_contains, name_matches) - "Remember fonts with 'Garamond'"
- Variable fonts only (is_variable) - "Just the flexible ones, thanks"
- Boolean logic (AND, OR, NOT) - "This but not that, or both actually"

## Behind the Curtain

This friendly wrapper introduces you to the Rust powerhouse in `typg_python`.
It's like having a bilingual friend who perfectly translates Python's casual charm
into Rust's lightning-fast intensity.

```bash
pip install typg
```

## Secret Weapons

Some friends bring extra toys:
- **hpindex**: Warp-speed searches through pre-built indexes

A lightweight ambassador that makes the Rust bindings feel right at home
in your cozy Python neighborhood.
"""

from __future__ import annotations

# Grab package version like sneaking a cookie from the jar
from importlib import metadata

# Import the workhorses: these functions actually do the heavy lifting
from typg_python import filter_cached, find, find_paths

# Public API - what we proudly show off to the world
__all__ = ["find", "find_paths", "filter_cached", "__version__"]

# Optional speed boosters (only available if built with hpindex feature)
try:
    from typg_python import count_indexed, find_indexed, list_indexed

    __all__.extend(["find_indexed", "list_indexed", "count_indexed"])
except ImportError:
    pass  # No hpindex feature? No worries, we've got your back anyway

# Version detection with fallback for development installs
try:
    __version__ = metadata.version(__name__)
except metadata.PackageNotFoundError:  # pragma: no cover - you're developing locally, cool!
    __version__ = "0.0.0"
