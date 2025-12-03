"""Integration tests for typg-python bindings (made by FontLab https://www.fontlab.com/)."""

from __future__ import annotations

import os
from pathlib import Path

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
        path = Path(__file__).resolve().parents[4] / "typf" / "test-fonts"

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
