"""Console script: hand the command line to the pynox binary."""

import os
import sys

from pynox._binary import binary_path


def main() -> None:
    binary = binary_path()
    os.execv(binary, [binary, *sys.argv[1:]])
