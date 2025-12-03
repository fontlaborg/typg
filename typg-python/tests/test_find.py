"""Integration tests for typg-python bindings (made by FontLab https://www.fontlab.com/)."""

from __future__ import annotations

import os
from pathlib import Path
from types import SimpleNamespace

import pytest

import typg_python
from typg_python import cli


@pytest.fixture(scope="session")
def fonts_dir() -> Path:
    env_override = os.getenv("TYPF_TEST_FONTS")
    if env_override:
        path = Path(env_override)
    else:
        # Repo root = .../github.fontlaborg; tests live under typg/typg-python/tests
        path = Path(__file__).resolve().parents[3] / "typf" / "test-fonts"

    if not path.exists():
        pytest.skip(f"test fonts missing at {path}")

    return path


def test_find_filters_variable_flag(fonts_dir: Path) -> None:
    variable_font = fonts_dir / "SourceSansVariable-Italic.otf"
    static_font = fonts_dir / "NotoSans-Regular.ttf"

    results = typg_python.find([str(variable_font), str(static_font)], variable=True)

    assert len(results) == 1, "only variable font should match the variable filter"
    match = results[0]
    assert match["path"].endswith("SourceSansVariable-Italic.otf")
    assert match["metadata"]["is_variable"] is True
    assert "wght" in match["metadata"]["axis_tags"], "variable font should expose axes"


def test_cli_uses_system_font_env_override(fonts_dir: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("TYPOG_SYSTEM_FONT_DIRS", str(fonts_dir))

    results = cli.find_cli(paths=None, system_fonts=True)

    assert results, "system font override should yield results"
    assert all(Path(item["path"]).exists() for item in results)


def test_find_accepts_jobs(fonts_dir: Path) -> None:
    results = typg_python.find([str(fonts_dir)], jobs=1)

    assert results, "jobs flag should not block search results"


def test_find_rejects_zero_jobs(fonts_dir: Path) -> None:
    with pytest.raises(ValueError):
        typg_python.find([str(fonts_dir)], jobs=0)


def test_find_paths_returns_strings_only(fonts_dir: Path) -> None:
    paths = typg_python.find_paths([str(fonts_dir)], scripts=["latn"])

    assert paths, "expected at least one path in paths-only mode"
    assert all(isinstance(path, str) for path in paths)
    assert all("metadata" not in path for path in paths)


def test_cli_paths_only_returns_paths(fonts_dir: Path) -> None:
    paths = cli.find_cli(paths=[fonts_dir], scripts=["latn"], paths_only=True)

    assert paths, "CLI paths_only should yield path strings"
    assert all(isinstance(path, str) for path in paths)
    assert all(path.endswith(('.otf', '.ttf', '.ttc')) for path in paths)


def _metadata(
    path: str,
    weight_class: int | None = None,
    width_class: int | None = None,
    family_class: tuple[int, int] | None = None,
) -> dict:
    raw_family = None
    if family_class is not None:
        raw_family = (family_class[0] << 8) | family_class[1]

    return SimpleNamespace(
        path=path,
        names=["Test"],
        axis_tags=[],
        feature_tags=[],
        script_tags=[],
        table_tags=[],
        codepoints=["A"],
        is_variable=False,
        ttc_index=None,
        weight_class=weight_class,
        width_class=width_class,
        family_class=raw_family,
    )


def test_filter_cached_handles_weight_and_width() -> None:
    entries = [
        _metadata("Thin.ttf", weight_class=250, width_class=3),
        _metadata("Regular.ttf", weight_class=400, width_class=5),
    ]

    matches = typg_python.filter_cached(
        entries,
        weight="300-450",
        width="4-6",
    )

    assert len(matches) == 1
    assert matches[0]["path"] == "Regular.ttf"

    none = typg_python.filter_cached(entries, weight="800")
    assert none == []


def test_filter_cached_family_class_filters_major_and_subclass() -> None:
    entries = [
        _metadata("Sans.ttf", family_class=(8, 11)),
        _metadata("Serif.ttf", family_class=(1, 0)),
    ]

    matches = typg_python.filter_cached(entries, family_class="sans")
    assert len(matches) == 1
    assert matches[0]["path"] == "Sans.ttf"

    subclass = typg_python.filter_cached(entries, family_class="8.11")
    assert len(subclass) == 1
    assert subclass[0]["path"] == "Sans.ttf"
