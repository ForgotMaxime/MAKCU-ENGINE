"""Release hygiene: single version source per stack, kept in sync."""

import re
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10: no stdlib tomllib
    tomllib = None  # type: ignore[assignment]

import makcu_engine

ROOT = Path(__file__).resolve().parent.parent


def _pyproject_version() -> str:
    path = ROOT / "pyproject.toml"
    if tomllib is not None:
        with open(path, "rb") as f:
            return tomllib.load(f)["project"]["version"]
    text = path.read_text(encoding="utf-8")
    m = re.search(r'^version\s*=\s*"([^"]+)"', text, re.MULTILINE)
    assert m, "version not found in pyproject.toml"
    return m.group(1)


def test_python_version_single_sourced():
    assert _pyproject_version() == makcu_engine.__version__


def test_rust_workspace_version_inherited():
    root = (ROOT / "rust" / "Cargo.toml").read_text()
    assert "[workspace.package]" in root
    m = re.search(r'\[workspace\.package\]\s*\nversion\s*=\s*"([^"]+)"', root)
    assert m, "workspace version missing"
    for crate in ("makcu_core", "makcu_serial"):
        content = (ROOT / "rust" / crate / "Cargo.toml").read_text()
        assert "version.workspace = true" in content, crate
