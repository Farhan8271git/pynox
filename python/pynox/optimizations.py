"""Transparent runtime optimizations, applied when their target module loads.

Pynox installs this once before user code runs. Nothing is imported or patched
until the application itself imports the optimized module, so programs that
never touch ``json`` pay no startup cost. Imports, names and call signatures in
application code remain ordinary Python.
"""

import sys


def _path_finder():
    # The interpreter's own path finder is already in sys.meta_path. Reusing it
    # avoids importing importlib.machinery (and warnings) at every startup.
    for finder in sys.meta_path:
        if getattr(finder, "__name__", None) == "PathFinder":
            return finder
    from importlib.machinery import PathFinder

    return PathFinder


class _PatchedLoader:
    """Execute the real stdlib module, then apply Pynox's patch to it once."""

    def __init__(self, loader, patch):
        self._loader = loader
        self._patch = patch

    def create_module(self, spec):
        return self._loader.create_module(spec)

    def exec_module(self, module):
        self._loader.exec_module(module)
        # Hand the module back exactly as CPython would describe it.
        module.__loader__ = self._loader
        if module.__spec__ is not None:
            module.__spec__.loader = self._loader
        self._patch.apply(module)

    def __getattr__(self, name):
        return getattr(self._loader, name)


class LazyPatch:
    """Meta path finder that patches ``target`` right after it is first executed."""

    def __init__(self, target, runtime_name, code):
        self.target = target
        self.runtime_name = runtime_name
        self.code = code

    def find_spec(self, fullname, path=None, target=None):
        if fullname != self.target:
            return None
        spec = _path_finder().find_spec(fullname, path)
        if spec is None or spec.loader is None:
            return None
        spec.loader = _PatchedLoader(spec.loader, self)
        return spec

    def apply(self, module):
        if self in sys.meta_path:
            sys.meta_path.remove(self)
        runtime = type(sys)(self.runtime_name)
        exec(self.code, runtime.__dict__)
        sys.modules[self.runtime_name] = runtime
        runtime.install(module)


def install(json_code):
    """Register the lazy patches. ``json_code`` is precompiled by the build."""
    patch = LazyPatch("json", "_pynox_json_runtime", json_code)
    if "json" in sys.modules:
        # Trusted startup code (for example a pool preload) imported json
        # already; patch it directly so every task still gets the same path.
        patch.apply(sys.modules["json"])
        return
    sys.meta_path.insert(0, patch)
