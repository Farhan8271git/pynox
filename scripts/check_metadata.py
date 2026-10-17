"""Validate versions, local documentation links and pinned workflow actions."""

import argparse
import re
import tomllib
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--tag")
    args = parser.parse_args()
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]
    project = tomllib.loads((ROOT / "pyproject.toml").read_text())["project"]
    version = cargo["version"]
    assert project["version"] == version
    assert f"PYNOX_VERSION:-{version}" in (ROOT / "scripts/install.sh").read_text()
    assert f"## {version}" in (ROOT / "CHANGELOG.md").read_text()
    if args.tag:
        assert args.tag == f"v{version}", (args.tag, version)
    for source in (ROOT / ".github").rglob("*.yml"):
        data = yaml.load(source.read_text(), Loader=yaml.BaseLoader)
        assert isinstance(data, dict), source
        if source.parent.name != "workflows":
            continue
        assert "on" in data and "permissions" in data, source
        for action in re.findall(r"^\s*(?:-\s*)?uses:\s*(\S+)", source.read_text(), re.M):
            assert action.startswith("./") or re.search(r"@[a-f0-9]{40}$", action), (source, action)
    for directory in (ROOT, ROOT / "docs", ROOT / "ADR"):
        for source in directory.glob("*.md"):
            for target in re.findall(r"\[[^]]+\]\(([^)]+)\)", source.read_text()):
                if "://" in target or target.startswith("#"):
                    continue
                destination = (source.parent / target.split("#")[0]).resolve()
                if destination.is_relative_to(ROOT / "docs/adr"):
                    destination = ROOT / "ADR" / destination.relative_to(ROOT / "docs/adr")
                assert destination.exists(), (source, target)
    print(f"Pynox {version}: metadata, links and action pins valid")


if __name__ == "__main__":
    main()
