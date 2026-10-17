"""Trusted worker definitions; _pynox_run needs an active sandbox.

This module deliberately imports only ``os`` and ``sys``, which CPython has
already initialized. Every additional import here is paid by every task.
"""

import os
import sys


def _pynox_env(environment):
    os.environ.clear()
    os.environ.update(environment)


def _pynox_run(script, args, package_dirs, site_packages, source):
    # Avoid importing site: .pth and sitecustomize must not run automatically.
    sys.path.extend(path for path in site_packages if path not in sys.path)
    sys.path.extend(package_dirs)
    if source is None:
        sys.path.insert(0, os.path.dirname(script))
    sys.argv = [script, *args]
    sys.dont_write_bytecode = True
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(line_buffering=True, write_through=True)
    exit_code = 0
    error = None
    module = type(sys)("__main__")
    if source is None:
        module.__file__ = script
    module.__package__ = None
    sys.modules["__main__"] = module
    try:
        if source is None:
            # compile(bytes) respects Python source encoding declarations.
            with open(script, "rb") as file:
                source = file.read()
        exec(compile(source, script, "exec"), module.__dict__)
    except SystemExit as exc:
        if exc.code is None:
            exit_code = 0
        elif isinstance(exc.code, int):
            exit_code = exc.code & 255
        else:
            exit_code = 1
            print(str(exc.code), file=sys.stderr)
    except BaseException as exc:
        line = None
        filename = None
        script_line = None
        tb = exc.__traceback__
        while tb is not None:
            filename = tb.tb_frame.f_code.co_filename
            line = tb.tb_lineno
            if filename == script:
                script_line = line
            tb = tb.tb_next
        if script_line is not None:
            filename, line = script, script_line
        if isinstance(exc, SyntaxError):
            line = exc.lineno
            filename = exc.filename
        hint = None
        if isinstance(exc, PermissionError):
            hint = (
                "For files, grant the specific path with --allow-read/--allow-write. "
                "Use --allow-network for IPv4/IPv6. Subprocesses and host Unix sockets are denied."
            )
        elif isinstance(exc, MemoryError):
            hint = "Reduce allocations or explicitly increase --memory."
        exit_code = 1
        error = {
            "type": type(exc).__name__,
            "message": str(exc)[:4096],
            "file": filename,
            "line": line,
            "path": getattr(exc, "filename", None),
            "hint": hint,
        }
    sys.stdout.flush()
    sys.stderr.flush()
    # The launcher serializes this report on the diagnostic descriptor. It is
    # not a tamper-proof audit log.
    return exit_code, error
