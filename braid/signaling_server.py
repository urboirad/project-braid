from __future__ import annotations

import json
from http import HTTPStatus
from http.server import ThreadingHTTPServer, BaseHTTPRequestHandler
from typing import Dict, Tuple


# In-memory stores
# - _SESSION_STORE: session_id -> manifest JSON string
# - _STATE_STORE:   session_id -> opaque save-state bytes (base64 encoded)
_SESSION_STORE: Dict[str, str] = {}
_STATE_STORE: Dict[str, str] = {}


class _SignalingHandler(BaseHTTPRequestHandler):
    server_version = "BraidSignaling/0.1"

    def _parse_path(self) -> Tuple[str | None, str | None]:
        """Return (resource, session_id) for known prefixes.

        Supported forms:
          /sessions/<session_id>
          /state/<session_id>
        """

        parts = self.path.split("?")[0].rstrip("/").split("/")
        if len(parts) == 3 and parts[2]:
            return parts[1], parts[2]
        return None, None

    def do_GET(self) -> None:  # noqa: N802
        resource, session_id = self._parse_path()
        if resource == "sessions" and session_id:
            manifest = _SESSION_STORE.get(session_id)
            if manifest is None:
                self.send_error(HTTPStatus.NOT_FOUND, "Session not found")
                return

            self.send_response(HTTPStatus.OK)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(manifest.encode("utf-8"))
            return

        if resource == "state" and session_id:
            state_b64 = _STATE_STORE.get(session_id)
            if state_b64 is None:
                self.send_error(HTTPStatus.NOT_FOUND, "State not found")
                return

            self.send_response(HTTPStatus.OK)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            payload = json.dumps({"session_id": session_id, "state": state_b64})
            self.wfile.write(payload.encode("utf-8"))
            return

        self.send_error(HTTPStatus.NOT_FOUND, "Unknown endpoint")

    def do_POST(self) -> None:  # noqa: N802
        resource, session_id = self._parse_path()
        if resource not in {"sessions", "state"} or not session_id:
            self.send_error(HTTPStatus.NOT_FOUND, "Unknown endpoint")
            return

        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length) if length > 0 else b""

        try:
            data = raw.decode("utf-8")
            body = json.loads(data)
        except Exception:
            self.send_error(HTTPStatus.BAD_REQUEST, "Invalid JSON body")
            return

        if resource == "sessions":
            # Body is raw manifest JSON; we simply store it.
            _SESSION_STORE[session_id] = data
        elif resource == "state":
            # Body should be {"state": base64_string}
            state_b64 = body.get("state")
            if not isinstance(state_b64, str):
                self.send_error(HTTPStatus.BAD_REQUEST, "Missing 'state' field")
                return
            _STATE_STORE[session_id] = state_b64

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
            - POST /state/<session_id>     (body: {"state": base64_string})
            - GET  /state/<session_id>     (returns same JSON)

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
