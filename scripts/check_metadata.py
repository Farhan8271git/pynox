"""Validate versions, local documentation links and pinned workflow actions."""

import argparse
import json
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
    pythons = (ROOT / ".python-versions").read_text().splitlines()
    assert [".".join(p.split(".")[:2]) for p in pythons] == [
        "3.11",
        "3.12",
        "3.13",
        "3.14",
        "3.15",
    ], pythons
    assert (ROOT / ".python-version").read_text().strip() == pythons[0]
    assert project["requires-python"] == ">=3.11,<3.16"
    images = json.loads((ROOT / ".ci/images.json").read_text())
    assert set(images) == {
        f"linux-{family}-{architecture}"
        for family in ("gnu", "musl")
        for architecture in ("x86_64", "aarch64")
    }
    assert all(re.search(r"@sha256:[a-f0-9]{64}$", image) for image in images.values())
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
    for directory in (ROOT, ROOT / "docs", ROOT / "ADR", ROOT / "tests"):
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
