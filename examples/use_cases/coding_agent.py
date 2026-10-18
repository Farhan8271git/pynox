"""A coding agent that runs the project's tests after editing it.

`pynox -m unittest` makes the project directory readable and nothing writable,
so a test, or code the model just wrote, cannot delete files or read secrets
outside the project, and cannot write inside it either.
"""

import os
import subprocess
import tempfile
from pathlib import Path

PROJECT = {
    "calc.py": "def add(a, b):\n    return a + b\n",
    "test_calc.py": (
        "import os, shutil, unittest\n"
        "from calc import add\n\n"
        "class CalcTest(unittest.TestCase):\n"
        "    def test_add(self):\n"
        "        self.assertEqual(add(2, 3), 5)\n\n"
        "    def test_cleanup_gone_wrong(self):\n"
        "        # A model 'cleaning up fixtures' with the wrong path.\n"
        "        shutil.rmtree(os.path.expanduser('~/important-notes'))\n"
    ),
}


def run(root: Path, binary: str) -> dict:
    project = root / "project"
    project.mkdir()
    for name, source in PROJECT.items():
        (project / name).write_text(source)
    notes = root / "home" / "important-notes"
    notes.mkdir(parents=True)
    (notes / "plan.txt").write_text("do not delete")
    env = {**os.environ, "HOME": str(root / "home")}
    result = subprocess.run(
        [binary, "--allow-env=HOME", "-m", "unittest", "-q"],
        cwd=project,
        env=env,
        capture_output=True,
        text=True,
        timeout=60,
    )
    return {
        "exit": result.returncode,
        "stderr": result.stderr,
        "notes_survived": (notes / "plan.txt").exists(),
    }


if __name__ == "__main__":
    with tempfile.TemporaryDirectory() as temporary:
        outcome = run(Path(temporary), os.environ.get("PYNOX_BIN", "pynox"))
        print(outcome["stderr"])
        print("exit", outcome["exit"], "| notes survived:", outcome["notes_survived"])
