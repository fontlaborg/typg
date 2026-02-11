"""
Command-line interface for Typg font discovery system.

Translates Python bindings into an intuitive Fire-based CLI that searches, filters
and matches fonts by criteria like axes, features, scripts, text support, and
metadata. Handles path resolution from multiple sources including stdin and system
directories.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path
from typing import Iterable, List, Sequence

import fire

from . import find, find_paths


def _dedup_paths(paths: Iterable[Path]) -> List[Path]:
    # Remove duplicate paths while preserving original order.
    # Uses set membership test for O(1) lookup complexity.
    seen: set[Path] = set()
    ordered: List[Path] = []
    for path in paths:
        if path not in seen:
            seen.add(path)
            ordered.append(path)
    return ordered


def _system_font_roots() -> List[Path]:
    # Return platform-specific system font directories.
    # Respects TYPOG_SYSTEM_FONT_DIRS env var or defaults to OS paths.
    env = os.getenv("TYPOG_SYSTEM_FONT_DIRS")
    if env:
        parts = [Path(p) for p in env.split(os.pathsep) if p]
        existing = [p for p in parts if p.exists()]
        if not existing:
            raise ValueError("TYPOG_SYSTEM_FONT_DIRS set but none of the paths exist")
        return _dedup_paths(existing)

    candidates: list[Path] = []
    if sys.platform == "darwin":
        candidates = [
            Path("/System/Library/Fonts"),
            Path("/Library/Fonts"),
            Path.home() / "Library" / "Fonts",
        ]
    elif sys.platform.startswith("linux"):
        candidates = [
            Path("/usr/share/fonts"),
            Path("/usr/local/share/fonts"),
            Path.home() / ".local" / "share" / "fonts",
        ]
    elif sys.platform == "win32":
        system_root = os.getenv("SYSTEMROOT")
        local_appdata = os.getenv("LOCALAPPDATA")
        if system_root:
            candidates.append(Path(system_root) / "Fonts")
        if local_appdata:
            candidates.append(Path(local_appdata) / "Microsoft" / "Windows" / "Fonts")

    existing = [p for p in candidates if p.exists()]
    if not existing:
        raise ValueError("no system font directories found")
    return _dedup_paths(existing)


def _gather_paths(
    paths: Sequence[Path | str] | None,
    stdin_paths: bool,
    include_system: bool,
) -> List[str]:
    # Consolidate font search paths from multiple sources.
    # Sources: CLI args, stdin (--), system directories.
    collected: list[Path] = []

    if stdin_paths:
        collected.extend(Path(line.strip()) for line in sys.stdin if line.strip())

    for p in paths or []:
        p = Path(p)
        if str(p) == "-":
            collected.extend(Path(line.strip()) for line in sys.stdin if line.strip())
        else:
            collected.append(p)

    if include_system:
        collected.extend(_system_font_roots())

    collected = [p for p in collected if str(p)]
    if not collected:
        raise ValueError("no search paths provided")

    return [str(p) for p in _dedup_paths(collected)]


def find_cli(
    paths: Sequence[Path | str] | None = None,
    axes: Sequence[str] | None = None,
    features: Sequence[str] | None = None,
    scripts: Sequence[str] | None = None,
    tables: Sequence[str] | None = None,
    names: Sequence[str] | None = None,
    codepoints: Sequence[str] | None = None,
    text: str | None = None,
    weight: str | None = None,
    width: str | None = None,
    family_class: str | None = None,
    variable: bool = False,
    follow_symlinks: bool = False,
    jobs: int | None = None,
    stdin_paths: bool = False,
    system_fonts: bool = False,
    paths_only: bool = False,
):
    """
    Fire-exposed command for font discovery.
    
    Accepts font filtering criteria and delegates to find() or find_paths()
    based on paths_only flag. All string sequences converted to lists.
    """

    gathered = _gather_paths(paths, stdin_paths, system_fonts)
    if paths_only:
        return find_paths(
            gathered,
            axes=list(axes) if axes else None,
            features=list(features) if features else None,
            scripts=list(scripts) if scripts else None,
            tables=list(tables) if tables else None,
            names=list(names) if names else None,
            codepoints=list(codepoints) if codepoints else None,
            text=text,
            weight=weight,
            width=width,
            family_class=family_class,
            variable=variable,
            follow_symlinks=follow_symlinks,
            jobs=jobs,
        )

    return find(
        gathered,
        axes=list(axes) if axes else None,
        features=list(features) if features else None,
        scripts=list(scripts) if scripts else None,
        tables=list(tables) if tables else None,
        names=list(names) if names else None,
        codepoints=list(codepoints) if codepoints else None,
        text=text,
        weight=weight,
        width=width,
        family_class=family_class,
        variable=variable,
        follow_symlinks=follow_symlinks,
        jobs=jobs,
    )


def main():
    # Entry point: Fire converts find_cli to command-line interface.
    fire.Fire({"find": find_cli})


if __name__ == "__main__":
    main()
