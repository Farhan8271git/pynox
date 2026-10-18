"""Monty orchestrates, Pynox executes: two sandboxes, one agent.

The model writes glue code that runs in Monty (pydantic-monty), a restricted
Python subset with no filesystem or network. When that code needs real Python
with libraries, it calls ``run_python(code)``: an external function the host
resolves on a Pynox pool. The host, not the model, decides what the heavy code
may read and write. Requires ``pip install pydantic-monty`` and Linux.
"""

from __future__ import annotations

import os
from pathlib import Path

import pydantic_monty
import pynox

# What an LLM would write for the task "average these readings and store them".
# It runs inside Monty; `run_python` is the only way out of its sandbox.
AGENT_CODE = """
import json

job = run_python(\'\'\'
import json, statistics
values = [3, 9, 27]
summary = {"mean": statistics.mean(values), "count": len(values)}
with open("{work_dir}/stats.json", "w") as f:
    json.dump(summary, f)
print(json.dumps(summary))
\'\'\')
if job["exit"] != 0:
    error = job["error"]
    raise RuntimeError(f"{error['type']} at line {error['line']}: {error['message']}")
json.loads(job["stdout"])["mean"]
"""


def run_agent(work_dir: str | os.PathLike[str], *, binary: str | os.PathLike[str] | None = None):
    work_dir = Path(work_dir)
    with pynox.Pool(workers=1, binary=binary) as pool, pydantic_monty.Monty() as monty:

        def run_python(code: str, timeout: str = "10s") -> dict:
            # Permissions are fixed here. The model never sees or chooses them.
            result = pool.run(
                code,
                name="heavy.py",
                allow_read=[work_dir],
                allow_write=[work_dir],
                timeout=timeout,
            )
            return {"exit": result.exit, "stdout": result.stdout, "error": result.error}

        with monty.checkout(script_name="agent.py") as session:
            return session.feed_run(
                AGENT_CODE.replace("{work_dir}", str(work_dir)),
                external_lookup={"run_python": run_python},
            )


if __name__ == "__main__":
    import tempfile

    with tempfile.TemporaryDirectory(prefix="pynox-monty-") as temporary:
        print("mean:", run_agent(temporary))
        print("stored:", (Path(temporary) / "stats.json").read_text())
