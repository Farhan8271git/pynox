"""Pynox: run Python written by agents in a kernel sandbox, from Python.

    import pynox

    result = pynox.run("print(sum(range(10)))")
    result.ok, result.text

    run_python = pynox.tool(profile="analysis")   # register in any agent framework

The package talks to the ``pynox pool`` process; permissions live in flags or
``pynox.toml``, never in the code the model writes.
"""

from pynox._binary import binary_path
from pynox._client import Pool, PoolError, Result
from pynox._tool import default_pool, run, tool

__version__ = "0.1.0"

__all__ = [
    "Pool",
    "PoolError",
    "Result",
    "__version__",
    "binary_path",
    "default_pool",
    "run",
    "tool",
]
