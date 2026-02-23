from __future__ import annotations

import json
from http import HTTPStatus
from http.server import ThreadingHTTPServer, BaseHTTPRequestHandler
from typing import Dict, Tuple


# In-memory store: session_id -> manifest JSON string
_SESSION_STORE: Dict[str, str] = {}


class _SignalingHandler(BaseHTTPRequestHandler):
    server_version = "BraidSignaling/0.1"

    def _parse_session_id(self) -> Tuple[bool, str | None]:
        parts = self.path.split("?")[0].rstrip("/").split("/")
        if len(parts) == 3 and parts[1] == "sessions" and parts[2]:
            return True, parts[2]
        return False, None

    def do_GET(self) -> None:  # noqa: N802
        ok, session_id = self._parse_session_id()
        if not ok or session_id is None:
            self.send_error(HTTPStatus.NOT_FOUND, "Unknown endpoint")
            return

        manifest = _SESSION_STORE.get(session_id)
        if manifest is None:
            self.send_error(HTTPStatus.NOT_FOUND, "Session not found")
            return

        self.send_response(HTTPStatus.OK)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(manifest.encode("utf-8"))

    def do_POST(self) -> None:  # noqa: N802
        ok, session_id = self._parse_session_id()
        if not ok or session_id is None:
            self.send_error(HTTPStatus.NOT_FOUND, "Unknown endpoint")
            return

        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length) if length > 0 else b""

        try:
            # We store the raw JSON string but validate it parses.
            data = raw.decode("utf-8")
            json.loads(data)
        except Exception:
            self.send_error(HTTPStatus.BAD_REQUEST, "Invalid JSON body")
            return

        _SESSION_STORE[session_id] = data

        self.send_response(HTTPStatus.CREATED)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(b"{}")

    def log_message(self, fmt: str, *args) -> None:  # silence default noisy logging
        return


def run_signaling_server(host: str = "0.0.0.0", port: int = 8080) -> None:
    """Run a tiny HTTP signaling server.

    Endpoints:
      - POST /sessions/<session_id>  (body: manifest JSON)
      - GET  /sessions/<session_id>  (returns manifest JSON)

    This is intentionally minimal and in-memory for development and LAN testing.
    """

    server = ThreadingHTTPServer((host, port), _SignalingHandler)
    print(f"[braid-signal] Listening on http://{host}:{port}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\n[braid-signal] Shutting down")
        server.server_close()


if __name__ == "__main__":  # pragma: no cover
    run_signaling_server()
