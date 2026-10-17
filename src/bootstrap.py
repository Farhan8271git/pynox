"""Only trusted definitions are preloaded; _pynox_run needs an active sandbox."""

import json
import os
import sys
import types


def _pynox_run(config, site_packages):
    script = config["script"]
    # Avoid importing site: .pth and sitecustomize must not run automatically.
    sys.path.extend(path for path in site_packages if path not in sys.path)
    sys.path.extend(config["package_dirs"])
    sys.path.insert(0, os.path.dirname(script))
    sys.argv = [script, *config["args"]]
    sys.dont_write_bytecode = True
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(line_buffering=True, write_through=True)
    report = {"exit": 0, "error": None}
    module = types.ModuleType("__main__")
    module.__file__ = script
    module.__package__ = None
    sys.modules["__main__"] = module
    try:
        # compile(bytes) respects Python source encoding declarations.
        with open(script, "rb") as source:
            code = compile(source.read(), script, "exec")
        exec(code, module.__dict__)
    except SystemExit as exc:
        if exc.code is None:
            report["exit"] = 0
        elif isinstance(exc.code, int):
            report["exit"] = exc.code & 255
        else:
            report["exit"] = 1
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
        report = {
            "exit": 1,
            "error": {
                "type": type(exc).__name__,
                "message": str(exc)[:4096],
                "file": filename,
                "line": line,
                "path": getattr(exc, "filename", None),
                "hint": hint,
            },
        }
    # This channel is for diagnostics, not a tamper-proof audit log.
    sys.stdout.flush()
    sys.stderr.flush()
    payload = json.dumps(report, ensure_ascii=True).encode()
    while payload:
        payload = payload[os.write(3, payload) :]
    return report["exit"]
