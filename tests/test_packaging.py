"""Release hygiene: single version source per stack, kept in sync."""

import re
from pathlib import Path

import tomllib

import makcu_engine

ROOT = Path(__file__).resolve().parent.parent


def test_python_version_single_sourced():
    with open(ROOT / "pyproject.toml", "rb") as f:
        pyproject = tomllib.load(f)
    assert pyproject["project"]["version"] == makcu_engine.__version__


def test_rust_workspace_version_inherited():
    root = (ROOT / "rust" / "Cargo.toml").read_text()
    assert '[workspace.package]' in root
    m = re.search(r'\[workspace\.package\]\s*\nversion\s*=\s*"([^"]+)"', root)
    assert m, "workspace version missing"
    for crate in ("makcu_core", "makcu_serial"):
        content = (ROOT / "rust" / crate / "Cargo.toml").read_text()
        assert 'version.workspace = true' in content, crate
