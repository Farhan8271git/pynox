"""Framework-agnostic execution tool: run agent-written Python on a Pynox pool.

Register ``PynoxPool.run`` as a tool in any agent framework that can call a
Python function. Each call runs in a fresh sandboxed process forked from a warm
CPython template, with the permissions *this host* chooses. The model only
supplies code. Linux only; see docs/usage.md for the pool protocol.

    with PynoxPool(workers=2, preload=["json"]) as pool:
        result = pool.run("print(sum(range(10)))")
        assert result.stdout == "45\\n"
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import threading
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable


@dataclass(frozen=True)
class ExecutionResult:
    """What the agent gets back: enough to decide the next step."""

    exit: int
    stdout: str
    stderr: str
    error: dict[str, Any] | None
    duration_ms: int
    truncated: bool

    @property
    def ok(self) -> bool:
        return self.exit == 0 and self.error is None

    def describe(self) -> str:
        """Plain text summary suitable for feeding back to a language model."""
        if self.ok:
            return self.stdout or "(no output)"
        if self.error is None:
            return f"exit status {self.exit}\n{self.stderr}".strip()
        parts = [f"{self.error['type']}: {self.error.get('message', '')}".strip()]
        if self.error.get("file") is not None and self.error.get("line") is not None:
            parts.append(f"at {self.error['file']}:{self.error['line']}")
        if self.error.get("hint"):
            parts.append(self.error["hint"])
        if self.stderr:
            parts.append(self.stderr.rstrip())
        return "\n".join(parts)


class PynoxPool:
    """One `pynox pool` process; `run` sends a sandboxed job and waits for its result.

    Calls are serialized with a lock, so a single instance is safe to share
    between threads. Use more instances for concurrent jobs.
    """

    def __init__(
        self,
        workers: int = 2,
        preload: Iterable[str] = (),
        package_dirs: Iterable[str | os.PathLike[str]] = (),
        binary: str | os.PathLike[str] | None = None,
    ):
        binary = binary or os.environ.get("PYNOX_BIN") or shutil.which("pynox")
        if binary is None:
            raise RuntimeError("pynox is not on PATH; set PYNOX_BIN or pass binary=")
        command = [str(binary), "pool", f"--workers={workers}"]
        preload = list(preload)
        if preload:
            command.append(f"--preload={','.join(preload)}")
        for directory in package_dirs:
            command.append(f"--package-dir={Path(directory)}")
        self._process = subprocess.Popen(
            command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True
        )
        ready = json.loads(self._process.stdout.readline() or "{}")
        if ready.get("event") != "ready":
            self.close()
            raise RuntimeError(f"Pynox pool did not start: {ready}")
        self._lock = threading.Lock()
        self._sequence = 0

    def run(
        self,
        code: str,
        *,
        name: str = "agent.py",
        args: Iterable[str] = (),
        allow_read: Iterable[str | os.PathLike[str]] = (),
        allow_write: Iterable[str | os.PathLike[str]] = (),
        package_dirs: Iterable[str | os.PathLike[str]] = (),
        allow_network: bool = False,
        env: dict[str, str] | None = None,
        timeout: str = "10s",
        memory: str = "512m",
        max_output: int = 65_536,
    ) -> ExecutionResult:
        request = {
            "code": code,
            "name": name,
            "args": list(args),
            "allow_read": [str(Path(p)) for p in allow_read],
            "allow_write": [str(Path(p)) for p in allow_write],
            "package_dirs": [str(Path(p)) for p in package_dirs],
            "allow_network": allow_network,
            "env": dict(env or {}),
            "timeout": timeout,
            "memory": memory,
            "max_output": max_output,
        }
        with self._lock:
            self._sequence += 1
            request["id"] = self._sequence
            self._process.stdin.write(json.dumps(request) + "\n")
            self._process.stdin.flush()
            line = self._process.stdout.readline()
        if not line:
            raise RuntimeError("Pynox pool exited")
        report = json.loads(line)
        if report.get("id") != request["id"]:
            raise RuntimeError(f"Pool answered another request: {report}")
        return ExecutionResult(
            exit=report["exit"],
            stdout=report.get("stdout", ""),
            stderr=report.get("stderr", ""),
            error=report.get("error"),
            duration_ms=report.get("duration_ms", 0),
            truncated=bool(report.get("stdout_truncated") or report.get("stderr_truncated")),
        )

    def close(self) -> None:
        if self._process.stdin and not self._process.stdin.closed:
            self._process.stdin.close()
        try:
            self._process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self._process.kill()
            self._process.wait()

    def __enter__(self) -> PynoxPool:
        return self

    def __exit__(self, *exc_info: object) -> None:
        self.close()


if __name__ == "__main__":
    with PynoxPool(workers=1) as pool:
        print(pool.run("import sys\nprint('Hello from an agent tool', sys.argv)").describe())
        print(pool.run("open('/etc/passwd').read()", name="probe.py").describe())
