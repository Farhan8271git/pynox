"""An agent that calls an external API with a key it never sees in its prompt.

The host forwards one environment variable and turns the network on for this
profile only. Without the grant, the same code cannot open a socket.
A local HTTP server stands in for the API.
"""

import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import pynox


class WeatherAPI(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        authorized = self.headers.get("Authorization") == "Bearer demo-key"
        body = json.dumps(
            {"city": "Lisbon", "temp_c": 21} if authorized else {"error": "unauthorized"}
        )
        self.send_response(200 if authorized else 401)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body.encode())


def model_code(port: int) -> str:
    # What a model writes for "What's the weather in Lisbon?"
    return (
        "import json, os, urllib.request\n"
        f"request = urllib.request.Request('http://127.0.0.1:{port}/weather?city=Lisbon',\n"
        "    headers={'Authorization': 'Bearer ' + os.environ['WEATHER_KEY']})\n"
        "print(json.load(urllib.request.urlopen(request, timeout=5)))\n"
    )


def run() -> dict:
    server = ThreadingHTTPServer(("127.0.0.1", 0), WeatherAPI)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        code = model_code(server.server_address[1])
        key = {"WEATHER_KEY": "demo-key"}
        offline = pynox.tool(env=key)(code)  # same key, no network grant
        online = pynox.tool(allow_network=True, env=key)(code)
    finally:
        server.shutdown()
    return {"offline": offline, "online": online}


if __name__ == "__main__":
    print(json.dumps(run(), indent=2))
