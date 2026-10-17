"""Copy canonical ADRs into the generated documentation input."""

import shutil
from pathlib import Path

root = Path(__file__).resolve().parents[1]
shutil.copytree(root / "ADR", root / "docs/adr", dirs_exist_ok=True)
