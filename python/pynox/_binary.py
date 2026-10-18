"""Locate the pynox executable this package drives."""

from __future__ import annotations

import os
import shutil
from pathlib import Path

BUNDLED = Path(__file__).resolve().parent / "_bundle" / "bin" / "pynox"


def binary_path() -> str:
    """The pynox binary: ``$PYNOX_BIN``, the wheel's bundled copy, or one on PATH."""
    explicit = os.environ.get("PYNOX_BIN")
    if explicit:
        return explicit
    if BUNDLED.is_file():
        return str(BUNDLED)
    found = shutil.which("pynox")
    if found:
        return found
    raise RuntimeError(
        "pynox binary not found: install the pynox wheel for your platform, "
        "put `pynox` on PATH, or set PYNOX_BIN"
    )
