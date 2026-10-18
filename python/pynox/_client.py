"""Client for the `pynox pool` JSON Lines protocol.

One ``Pool`` owns one ``pynox pool`` process. ``run`` sends a request and
waits for its result; a reader thread routes results back by ``id``, so calls
from several threads proceed concurrently up to the pool's worker count.
"""

from __future__ import annotations

import json
import queue
import subprocess
import threading
import uuid
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable, Mapping

from pynox._binary import binary_path

PathLike = "str | os.PathLike[str]"


class PoolError(RuntimeError):
    """The pool process is unavailable or answered out of protocol."""


@dataclass(frozen=True)
class Result:
    """What one execution produced. ``text`` is the summary a model should read."""

    exit: int
    stdout: str
    stderr: str
    error: dict[str, Any] | None
    duration_ms: int
    truncated: bool
    policy: dict[str, Any] = field(default_factory=dict, repr=False)
    raw: dict[str, Any] = field(default_factory=dict, repr=False)

    @property
    def ok(self) -> bool:
        return self.exit == 0 and self.error is None

    @property
    def text(self) -> str:
        """Plain text for a language model: output, or what went wrong and where."""
        if self.ok:
            return self.stdout if self.stdout else "(no output)"
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

    def describe(self) -> str:
        return self.text


class Pool:
    """A `pynox pool` process. Starts lazily on first use; close it or use ``with``.

    Permissions are decided here or in ``pynox.toml``, per call or per profile,
    by the application. The code passed to ``run`` only ever supplies code.
    """

    def __init__(
        self,
        workers: int | None = None,
        *,
        preload: Iterable[str] = (),
        package_dirs: Iterable[PathLike] = (),
        config: PathLike | None = None,
        binary: PathLike | None = None,
    ):
        self._command = [str(binary or binary_path()), "pool"]
        if workers is not None:
            self._command.append(f"--workers={workers}")
        preload = list(preload)
        if preload:
            self._command.append(f"--preload={','.join(preload)}")
        for directory in package_dirs:
            self._command.append(f"--package-dir={Path(directory)}")
        if config is not None:
            self._command.append(f"--config={Path(config)}")
        self._process: subprocess.Popen[str] | None = None
        self._closed = False
        self._lock = threading.Lock()
        self._pending: dict[int, queue.Queue[dict[str, Any] | None]] = {}
        self._sequence = 0
        self.ready: dict[str, Any] = {}

    # Lifecycle ----------------------------------------------------------

    def start(self) -> Pool:
        with self._lock:
            if self._process is not None:
                return self
            if self._closed:
                raise PoolError("pynox pool is closed")
            process = subprocess.Popen(
                self._command,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                text=True,
                bufsize=1,
            )
            first = process.stdout.readline() if process.stdout else ""
            try:
                ready = json.loads(first) if first else {}
            except json.JSONDecodeError:
                ready = {}
            if ready.get("event") != "ready":
                process.kill()
                process.wait()
                raise PoolError(f"pynox pool did not start: {ready.get('message') or first!r}")
            self._process = process
            self.ready = ready
            threading.Thread(target=self._reader, name="pynox-pool-reader", daemon=True).start()
            return self

    def close(self) -> None:
        with self._lock:
            process, self._process = self._process, None
            self._closed = True
        if process is None:
            return
        if process.stdin and not process.stdin.closed:
            process.stdin.close()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()

    def __enter__(self) -> Pool:
        return self.start()

    def __exit__(self, *exc_info: object) -> None:
        self.close()

    def __del__(self) -> None:  # pragma: no cover - best effort
        try:
            self.close()
        except Exception:
            pass

    # Execution ----------------------------------------------------------

    def run(
        self,
        code: str,
        *,
        name: str = "agent.py",
        args: Iterable[str] = (),
        profile: str | None = None,
        allow_read: Iterable[PathLike] = (),
        allow_write: Iterable[PathLike] = (),
        package_dirs: Iterable[PathLike] = (),
        allow_network: bool | None = None,
        env: Mapping[str, str] | None = None,
        timeout: str | None = None,
        memory: str | None = None,
        max_output: int | None = None,
        max_file_size: str | None = None,
    ) -> Result:
        """Run ``code`` in a fresh sandboxed process and wait for its result.

        Fields left unset keep the pool's defaults, the profile's values and
        the operator's ``[pool.limits]``.
        """
        request: dict[str, Any] = {"code": code, "name": name, "args": list(args)}
        if profile is not None:
            request["profile"] = profile
        for key, paths in (
            ("allow_read", allow_read),
            ("allow_write", allow_write),
            ("package_dirs", package_dirs),
        ):
            paths = [str(Path(p)) for p in paths]
            if paths:
                request[key] = paths
        if allow_network is not None:
            request["allow_network"] = allow_network
        if env:
            request["env"] = dict(env)
        for key, value in (
            ("timeout", timeout),
            ("memory", memory),
            ("max_output", max_output),
            ("max_file_size", max_file_size),
        ):
            if value is not None:
                request[key] = value
        return self.request(request)

    def request(self, request: dict[str, Any]) -> Result:
        """Send a raw pool request (see the pool protocol) and wait for its result."""
        self.start()
        answers: queue.Queue[dict[str, Any] | None] = queue.Queue(maxsize=1)
        with self._lock:
            process = self._process
            if process is None or process.stdin is None:
                raise PoolError("pynox pool is closed")
            self._sequence += 1
            request_id = self._sequence
            self._pending[request_id] = answers
            try:
                process.stdin.write(json.dumps({**request, "id": request_id}) + "\n")
                process.stdin.flush()
            except (BrokenPipeError, ValueError) as error:
                self._pending.pop(request_id, None)
                raise PoolError("pynox pool exited") from error
        report = answers.get()
        if report is None:
            raise PoolError("pynox pool exited before answering")
        return Result(
            exit=report["exit"],
            stdout=report.get("stdout", ""),
            stderr=report.get("stderr", ""),
            error=report.get("error"),
            duration_ms=report.get("duration_ms", 0),
            truncated=bool(report.get("stdout_truncated") or report.get("stderr_truncated")),
            policy=report.get("policy", {}),
            raw=report,
        )

    def tool(self, profile: str | None = None, **options: Any):
        """A ``run_python(code) -> str`` function bound to this pool; see ``pynox.tool``."""
        from pynox._tool import make_tool

        return make_tool(self, profile=profile, **options)

    def session(self, name: str | None = None, **grants: Any) -> Session:
        """A persistent sandboxed process: variables survive between ``run`` calls.

        ``grants`` are ``run``'s permission options and apply to the whole
        session; they are sent with the first call and cannot change later.
        """
        return Session(self, name or f"session-{uuid.uuid4().hex[:12]}", grants)

    # Internals ----------------------------------------------------------

    def _reader(self) -> None:
        process = self._process
        stdout = process.stdout if process else None
        try:
            while stdout is not None:
                line = stdout.readline()
                if not line:
                    break
                try:
                    report = json.loads(line)
                except json.JSONDecodeError:
                    continue
                waiter = (
                    self._pending.pop(report.get("id"), None) if isinstance(report, dict) else None
                )
                if waiter is not None:
                    waiter.put(report)
        finally:
            with self._lock:
                pending, self._pending = self._pending, {}
            for waiter in pending.values():
                waiter.put(None)


class Session:
    """One isolated process whose ``__main__`` persists across ``run`` calls.

    Errors in a step leave the session alive; a timeout, a crash or ``close``
    end it, after which ``alive`` is False and the next ``run`` starts a new
    process under the same name and grants.
    """

    def __init__(self, pool: Pool, name: str, grants: dict[str, Any]):
        self.pool = pool
        self.name = name
        self._grants = grants
        self.alive = False
        self.steps = 0

    def run(
        self,
        code: str,
        *,
        name: str = "agent.py",
        args: Iterable[str] = (),
        timeout: str | None = None,
        max_output: int | None = None,
    ) -> Result:
        request: dict[str, Any] = {
            "session": self.name,
            "code": code,
            "name": name,
            "args": list(args),
        }
        if not self.alive:
            request.update(_grant_fields(self._grants))
        if timeout is not None:
            request["timeout"] = timeout
        if max_output is not None:
            request["max_output"] = max_output
        result = self.pool.request(request)
        self.steps += 1
        ended = result.raw.get("session_ended", False) if result.raw else False
        self.alive = result.error is None or result.error.get("type") not in (
            "RequestError",
            "PoolWorkerError",
        )
        if ended:
            self.alive = False
        return result

    def close(self) -> None:
        if self.alive:
            self.pool.request({"session": self.name, "close": True})
        self.alive = False

    def __enter__(self) -> Session:
        return self

    def __exit__(self, *exc_info: object) -> None:
        self.close()


def _grant_fields(grants: Mapping[str, Any]) -> dict[str, Any]:
    fields: dict[str, Any] = {}
    for key in ("allow_read", "allow_write", "package_dirs"):
        paths = [str(Path(p)) for p in grants.get(key, ())]
        if paths:
            fields[key] = paths
    for key in ("profile", "allow_network", "timeout", "memory", "max_output", "max_file_size"):
        if grants.get(key) is not None:
            fields[key] = grants[key]
    if grants.get("env"):
        fields["env"] = dict(grants["env"])
    return fields
