"""Identical behavioral probes for stock CPython and the experimental launcher."""

import builtins
import io
import json
import math
import os
import random
import sys
import tempfile
from pathlib import Path

RESULTS = {}
PATHS = {}


def counts():
    module = sys.modules.get("_pynox_json")
    return module.stats() if module else {"fast": 0, "fallback": 0}


def record(name, operation):
    before = counts()
    RESULTS[name] = operation()
    after = counts()
    PATHS[name] = {k: after[k] - before[k] for k in before}


def error(exc):
    return {"type": type(exc).__name__, "message": str(exc)}


def dumping(root, value, options=None, *, kind="file", encoding="utf-8", mode="w"):
    path = root / "output.json"
    path.write_bytes(b"")
    if kind == "string":
        fp = io.StringIO()
    elif kind == "binary":
        fp = open(path, "wb")
    elif kind == "custom":

        class Writer:
            def __init__(self):
                self.chunks = []

            def write(self, text):
                self.chunks.append(text)
                if len(self.chunks) == 5:
                    raise OSError("controlled writer failure")

        fp = Writer()
    elif kind == "closed":
        fp = open(path, "w")
        fp.close()
    elif kind == "monkey":
        fp = open(path, "w", encoding=encoding)
        original = fp.write
        fp.calls = []

        def write(text):
            fp.calls.append(text)
            return original(text)

        fp.write = write
    else:
        fp = open(path, mode, encoding=encoding, newline="", buffering=1 if kind == "line" else -1)
    result = {}
    try:
        result["return"] = json.dump(value, fp, **(options or {}))
    except Exception as exc:
        result["error"] = error(exc)
    if kind == "string":
        result["output"] = fp.getvalue()
    elif kind == "custom":
        result["chunks"] = fp.chunks
    else:
        if kind == "monkey":
            result["chunks"] = fp.calls
        fp.close()
        result["output_hex"] = path.read_bytes().hex()
    return result


def main():
    with tempfile.TemporaryDirectory(prefix="pynox-json-contract-") as directory:
        root = Path(directory)
        values = {
            "empty_dict": {},
            "empty_list": [],
            "none": None,
            "bool": True,
            "scalar_int": 42,
            "int64_edges": [-(2**63), 2**63 - 1],
            "unicode": {"café": ["☕", "🙂", "\n\r\t", "\x00", '\\"', "\ud800"]},
            "floats": [0.0, -0.0, 1e-320, 1e300, math.inf, -math.inf, math.nan],
            "nested": {"items": [{"price": 31, "quantity": 7}, {"price": 4, "quantity": 1}]},
            "tuples": (1, ("two", None)),
            "bigint": {"first": 1, "big": 2**100},
            "huge_int_error": {"first": 1, "big": 1 << 20000},
            "integer_keys": {1: "one", None: "none", False: "false"},
            "invalid_key": {"first": 1, (1, 2): "bad"},
            "unsupported_value": {"first": 1, "bad": {1, 2}},
        }
        alias = [1, 2, 3]
        values["shared_container"] = [alias, alias]
        cycle = [1]
        cycle.append(cycle)
        values["cycle"] = cycle
        deep = []
        for _ in range(70):
            deep = [deep]
        values["deep"] = deep
        for name, value in values.items():
            record(name, lambda value=value: dumping(root, value))
        corpus = {"b": [1, True, None, "á"], "a": {"nested": 2.5}}
        options = [
            {"ensure_ascii": False},
            {"indent": 2},
            {"indent": "\t"},
            {"indent": 0},
            {"separators": (",", ":")},
            {"sort_keys": True},
            {"skipkeys": True},
            {"allow_nan": False},
            {"check_circular": False},
            {"unknown": 1},
            {"ensure_ascii": 1},
            {"skipkeys": 0},
        ]
        for i, settings in enumerate(options):
            record(f"options_{i}", lambda settings=settings: dumping(root, corpus, settings))
        record(
            "nan_error", lambda: dumping(root, {"first": 1, "bad": math.nan}, {"allow_nan": False})
        )
        record("cycle_no_check", lambda: dumping(root, cycle, {"check_circular": False}))

        class Encoder(json.JSONEncoder):
            def default(self, obj):
                return "converted"

        record("encoder_class", lambda: dumping(root, {"obj": object()}, {"cls": Encoder}))
        record(
            "default_hook",
            lambda: dumping(root, {"obj": object()}, {"default": lambda obj: "converted"}),
        )

        class Int(int):
            pass

        class String(str):
            pass

        class List(list):
            pass

        for name, value in [
            ("int_subclass", Int(3)),
            ("str_subclass", String("x")),
            ("list_subclass", List([1, 2])),
        ]:
            record(name, lambda value=value: dumping(root, value))
        for kind in ("string", "custom", "binary", "closed", "monkey", "line"):
            record("writer_" + kind, lambda kind=kind: dumping(root, corpus, kind=kind))
        for encoding in ("ascii", "latin-1", "utf-16", "utf-8-sig"):
            record(
                "encoding_" + encoding,
                lambda encoding=encoding: dumping(root, corpus, encoding=encoding),
            )
        for mode in ("a", "w+"):
            record("mode_" + mode, lambda mode=mode: dumping(root, corpus, mode=mode))
        original = json.JSONEncoder.iterencode

        def iterencode(self, value, *args, **kwargs):
            yield '"custom-encoder"'

        json.JSONEncoder.iterencode = iterencode
        try:
            record("patched_encoder", lambda: dumping(root, corpus))
        finally:
            json.JSONEncoder.iterencode = original
        limit = sys.getrecursionlimit()
        try:
            sys.setrecursionlimit(100)
            record("low_recursion_limit", lambda: dumping(root, corpus))
        finally:
            sys.setrecursionlimit(limit)
        rng = random.Random(913)
        for i in range(100):
            value = [
                {
                    "id": j,
                    "name": rng.choice(["", "café", "\udc00", "中文"]),
                    "integer": rng.randrange(-(10**12), 10**12),
                    "float": rng.uniform(-1e20, 1e20),
                    "flags": [None, True, False],
                }
                for j in range(rng.randrange(0, 20))
            ]
            record(f"random_{i}", lambda value=value: dumping(root, value))

        # open and JSON reading remain the original implementations.
        RESULTS["open_identity"] = builtins.open is io.open
        path = root / "file.txt"
        with open(path, "w", encoding="utf-8", newline="") as fp:
            fp.write("café\r\nsecond\n")
            fp.flush()
            position = fp.tell()
            types = [type(fp).__name__, type(fp.buffer).__name__, type(fp.buffer.raw).__name__]
        with open(path, encoding="utf-8") as fp:
            first = fp.readline()
            fp.seek(0)
            text = fp.read()
        fd = os.open(path, os.O_RDONLY)
        with open(fd, "rb", closefd=False) as fp:
            raw = fp.read()
        still_open = os.fstat(fd).st_size
        os.close(fd)
        RESULTS["open_behavior"] = [position, types, first, text, raw.hex(), still_open]
        for i, raw in enumerate(
            [b'{"x":1}', '{"x":"á"}'.encode("utf-16"), b'{"x":', b'{"x":NaN}', b'{"x":1,"x":2}']
        ):
            path.write_bytes(raw)
            try:
                with open(path, "rb") as fp:
                    value = json.load(fp, object_pairs_hook=list, parse_int=str)
                RESULTS[f"load_{i}"] = repr(value)
            except Exception as exc:
                RESULTS[f"load_{i}"] = error(exc)
    print(json.dumps({"results": RESULTS, "paths": PATHS}, ensure_ascii=True))


if __name__ == "__main__":
    main()
