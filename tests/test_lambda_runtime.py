"""The Lambda layer's runtime loop against a local stand-in for the Runtime API."""

import json
import os
import subprocess
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import test_runtime as runtime

RUNTIME = runtime.ROOT / "deploy/lambda/runtime.py"


class FakeRuntimeAPI:
    """Serve queued events on /next and record what the runtime posts back."""

    def __init__(self, events):
        self.events = list(events)
        self.posts = []
        self.done = threading.Event()
        api = self

        class Handler(BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"

            def log_message(self, *args):
                pass

            def _send(self, status, body=b"", headers=()):
                self.send_response(status)
                for name, value in headers:
                    self.send_header(name, value)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def do_GET(self):
                if not api.events:
                    api.done.set()
                    threading.Event().wait(30)  # Lambda freezes the process here
                    return
                request_id, event = api.events.pop(0)
                self._send(
                    200,
                    json.dumps(event).encode(),
                    [
                        ("Lambda-Runtime-Aws-Request-Id", request_id),
                        ("Lambda-Runtime-Deadline-Ms", "9999999999999"),
                        ("Lambda-Runtime-Invoked-Function-Arn", "arn:test"),
                    ],
                )

            def do_POST(self):
                length = int(self.headers.get("Content-Length", "0"))
                api.posts.append((self.path, json.loads(self.rfile.read(length) or b"null")))
                self._send(202)
                if self.path.endswith("/init/error"):
                    api.done.set()

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=self.server.serve_forever, daemon=True).start()

    @property
    def address(self):
        return f"127.0.0.1:{self.server.server_address[1]}"

    def close(self):
        self.server.shutdown()


class LambdaRuntimeTests(unittest.TestCase):
    def run_runtime(self, handler_source, events, handler="handler.handler"):
        api = FakeRuntimeAPI(events)
        self.addCleanup(api.close)
        with tempfile.TemporaryDirectory(prefix="pynox-lambda-") as task_root:
            Path(task_root, "handler.py").write_text(handler_source)
            env = {
                "AWS_LAMBDA_RUNTIME_API": api.address,
                "LAMBDA_TASK_ROOT": task_root,
                "_HANDLER": handler,
                "AWS_LAMBDA_FUNCTION_NAME": "probe",
                "PATH": os.environ["PATH"],
            }
            process = subprocess.Popen(
                [str(runtime.BINARY), "--no-sandbox", "-I", str(RUNTIME)],
                env=env,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
            try:
                api.done.wait(20)
            finally:
                process.kill()
                _, stderr = process.communicate()
        return api.posts, stderr

    def test_events_results_context_and_errors(self):
        posts, stderr = self.run_runtime(
            "import json\n"
            "def handler(event, context):\n"
            "    if event.get('fail'):\n"
            "        raise ValueError('bad input')\n"
            "    return {'echo': event, 'id': context.aws_request_id,\n"
            "            'name': context.function_name,\n"
            "            'remaining_positive': context.get_remaining_time_in_millis() > 0}\n",
            [("r1", {"n": 1}), ("r2", {"fail": True}), ("r3", {"n": [1, 2]})],
        )
        self.assertEqual(
            [path for path, _ in posts],
            [
                "/2018-06-01/runtime/invocation/r1/response",
                "/2018-06-01/runtime/invocation/r2/error",
                "/2018-06-01/runtime/invocation/r3/response",
            ],
            stderr,
        )
        self.assertEqual(
            posts[0][1], {"echo": {"n": 1}, "id": "r1", "name": "probe", "remaining_positive": True}
        )
        self.assertEqual(
            (posts[1][1]["errorType"], posts[1][1]["errorMessage"]), ("ValueError", "bad input")
        )
        self.assertTrue(any("bad input" in line for line in posts[1][1]["stackTrace"]))
        self.assertEqual(posts[2][1]["echo"], {"n": [1, 2]})

    def test_import_failure_is_reported_as_init_error(self):
        posts, _ = self.run_runtime("raise ImportError('missing dependency')\n", [])
        self.assertEqual(posts[0][0], "/2018-06-01/runtime/init/error")
        self.assertEqual(posts[0][1]["errorMessage"], "missing dependency")

    def test_runtime_imports_stay_small(self):
        probe = subprocess.run(
            [
                str(runtime.BINARY),
                "--no-sandbox",
                "-I",
                "-c",
                "import runpy, sys\n"
                f"sys.argv = [{str(RUNTIME)!r}]\n"
                f"runpy.run_path({str(RUNTIME)!r}, run_name='not_main')\n"
                "print(sorted(m for m in ('urllib', 'http', 'email', 'ssl', 'traceback') if m in sys.modules))",
            ],
            capture_output=True,
            text=True,
            check=True,
        )
        self.assertEqual(probe.stdout.strip(), "[]")
