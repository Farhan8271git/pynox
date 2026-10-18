"""Give an agent a Python tool with the `pynox` package.

Register the function `pynox.tool()` returns in any framework that accepts a
typed, documented Python function. Each call runs in a fresh sandboxed
process; the host decides the permissions here, the model only writes code.

    PYNOX_BIN=$PWD/target/release/pynox uv run python examples/agent_tool.py
"""

import pynox

# One pool for the application. Permissions per call, or per profile in pynox.toml.
run_python = pynox.tool(allow_write=["/tmp"], timeout="10s", memory="256m")

if __name__ == "__main__":
    print(run_python.__doc__)
    print(run_python("import sys\nprint('Hello from an agent tool', sys.argv)"))
    print(run_python("open('/etc/passwd').read()"))
