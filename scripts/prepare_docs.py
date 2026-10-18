"""Copy canonical ADRs into the generated documentation input."""

import shutil
from pathlib import Path

root = Path(__file__).resolve().parents[1]
generated = root / "docs/adr"
# Start from a clean copy so renamed or removed records do not linger in the site.
shutil.rmtree(generated, ignore_errors=True)
shutil.copytree(root / "ADR", generated)
