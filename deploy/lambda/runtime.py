"""AWS Lambda Runtime API loop for Pynox, the `provided.al2023` way.

_HANDLER is `module.function`, imported from $LAMBDA_TASK_ROOT. The function
receives (event, context) like the managed Python runtimes and returns a
JSON-serializable value.

The Runtime API is plain HTTP/1.1 on a local address, so this file speaks it
over one keep-alive socket instead of importing urllib. On a cold start the
first read of every module costs real time: urllib.request alone pulls in
http.client, email and ssl, about 300 ms measured on Lambda.
"""

import json
import os
import socket
import sys
import time


class RuntimeAPI:
    def __init__(self, address):
        host, _, port = address.rpartition(":")
        self.address = (host, int(port))
        self.host = address
        self.sock = None
        self.buffer = b""

    def _connect(self):
        self.sock = socket.create_connection(self.address)
        self.buffer = b""

    def request(self, method, path, body=b""):
        for attempt in (0, 1):
            try:
                if self.sock is None:
                    self._connect()
                head = (
                    f"{method} /2018-06-01/runtime/{path} HTTP/1.1\r\n"
                    f"Host: {self.host}\r\nContent-Length: {len(body)}\r\n"
                    "Content-Type: application/json\r\n\r\n"
                ).encode()
                self.sock.sendall(head + body)
                return self._response()
            except (ConnectionError, OSError):
                # The API may close an idle keep-alive connection; retry once.
                self.sock = None
                if attempt:
                    raise

    def _read(self, size):
        while len(self.buffer) < size:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise ConnectionError("Runtime API closed the connection")
            self.buffer += chunk
        data, self.buffer = self.buffer[:size], self.buffer[size:]
        return data

    def _response(self):
        while b"\r\n\r\n" not in self.buffer:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise ConnectionError("Runtime API closed the connection")
            self.buffer += chunk
        head, self.buffer = self.buffer.split(b"\r\n\r\n", 1)
        lines = head.decode("latin-1").split("\r\n")
        status = int(lines[0].split()[1])
        headers = {}
        for line in lines[1:]:
            name, _, value = line.partition(":")
            headers[name.strip().lower()] = value.strip()
        body = self._read(int(headers.get("content-length", "0")))
        if headers.get("connection", "").lower() == "close":
            self.sock.close()
            self.sock = None
        return status, headers, body


class Context:
    def __init__(self, headers):
        self.aws_request_id = headers.get("lambda-runtime-aws-request-id")
        self.invoked_function_arn = headers.get("lambda-runtime-invoked-function-arn")
        self.function_name = os.environ.get("AWS_LAMBDA_FUNCTION_NAME")
        self.function_version = os.environ.get("AWS_LAMBDA_FUNCTION_VERSION")
        self.memory_limit_in_mb = os.environ.get("AWS_LAMBDA_FUNCTION_MEMORY_SIZE")
        self.log_group_name = os.environ.get("AWS_LAMBDA_LOG_GROUP_NAME")
        self.log_stream_name = os.environ.get("AWS_LAMBDA_LOG_STREAM_NAME")
        self._deadline_ms = int(headers.get("lambda-runtime-deadline-ms", "0"))

    def get_remaining_time_in_millis(self):
        return max(0, self._deadline_ms - int(time.time() * 1000))


def error_body(error):
    import traceback  # only paid when something fails

    return json.dumps(
        {
            "errorType": type(error).__name__,
            "errorMessage": str(error),
            "stackTrace": traceback.format_exception(error),
        }
    ).encode()


def load_handler():
    import importlib

    root = os.environ.get("LAMBDA_TASK_ROOT", "/var/task")
    sys.path.insert(0, root)
    module_name, _, function_name = os.environ.get("_HANDLER", "handler.handler").rpartition(".")
    return getattr(importlib.import_module(module_name), function_name)


def main():
    api = RuntimeAPI(os.environ["AWS_LAMBDA_RUNTIME_API"])
    try:
        handler = load_handler()
    except Exception as error:
        api.request("POST", "init/error", error_body(error))
        raise SystemExit(1)
    while True:
        _, headers, body = api.request("GET", "invocation/next")
        request_id = headers["lambda-runtime-aws-request-id"]
        try:
            event = json.loads(body or b"null")
            result = handler(event, Context(headers))
            api.request("POST", f"invocation/{request_id}/response", json.dumps(result).encode())
        except Exception as error:
            api.request("POST", f"invocation/{request_id}/error", error_body(error))


if __name__ == "__main__":
    main()
