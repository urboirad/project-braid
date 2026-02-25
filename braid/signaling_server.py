from __future__ import annotations

import json
from http import HTTPStatus
from http.server import ThreadingHTTPServer, BaseHTTPRequestHandler
from typing import Dict, Tuple, Any, List


# In-memory stores
# - _SESSION_STORE: session_id -> manifest JSON string
# - _STATE_STORE:   session_id -> opaque save-state bytes (base64 encoded)
_SESSION_STORE: Dict[str, str] = {}
_STATE_STORE: Dict[str, str] = {}
# - _WEBRTC_OFFERS:  session_id -> arbitrary JSON offer blob (typically from host)
# - _WEBRTC_ANSWERS: session_id -> arbitrary JSON answer blob (typically from peer)
# - _RELAY_QUEUES:   session_id -> client_id -> list of queued messages
_WEBRTC_OFFERS: Dict[str, Any] = {}
_WEBRTC_ANSWERS: Dict[str, Any] = {}
_RELAY_QUEUES: Dict[str, Dict[str, List[Dict[str, Any]]]] = {}


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
        # First handle WebRTC signaling and relay endpoints, which may have
        # deeper path structures than the simple /sessions and /state APIs.
        path = self.path.split("?")[0].rstrip("/")
        parts = path.split("/")

        # /webrtc/<session_id>/<role>
        if len(parts) == 4 and parts[1] == "webrtc":
            _, _, session_id, role = parts

            if role not in {"host", "peer"}:
                self.send_error(HTTPStatus.BAD_REQUEST, "Invalid WebRTC role")
                return

            store = _WEBRTC_OFFERS if role == "host" else _WEBRTC_ANSWERS
            blob = store.get(session_id)
            if blob is None:
                self.send_error(HTTPStatus.NOT_FOUND, "No WebRTC description for session")
                return

            self.send_response(HTTPStatus.OK)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            payload = json.dumps({"session_id": session_id, "role": role, "data": blob})
            self.wfile.write(payload.encode("utf-8"))
            return

        # /relay/<session_id>/<client_id>
        if len(parts) == 4 and parts[1] == "relay":
            _, _, session_id, client_id = parts
            session_queues = _RELAY_QUEUES.get(session_id, {})
            queue = session_queues.get(client_id, [])

            # Pop all pending messages for this client.
            if queue:
                messages = list(queue)
                session_queues[client_id] = []
            else:
                messages = []

            self.send_response(HTTPStatus.OK)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            payload = json.dumps({"session_id": session_id, "client_id": client_id, "messages": messages})
            self.wfile.write(payload.encode("utf-8"))
            return

        # Fallback to simple JSON APIs for manifests and small save-state blobs.
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
        # WebRTC signaling: POST /webrtc/<session_id>/<role>
        path = self.path.split("?")[0].rstrip("/")
        parts = path.split("/")

        if len(parts) == 4 and parts[1] == "webrtc":
            _, _, session_id, role = parts

            if role not in {"host", "peer"}:
                self.send_error(HTTPStatus.BAD_REQUEST, "Invalid WebRTC role")
                return

            length = int(self.headers.get("Content-Length", "0"))
            raw = self.rfile.read(length) if length > 0 else b""

            try:
                body = json.loads(raw.decode("utf-8")) if raw else {}
            except Exception:
                self.send_error(HTTPStatus.BAD_REQUEST, "Invalid JSON body")
                return

            store = _WEBRTC_OFFERS if role == "host" else _WEBRTC_ANSWERS
            store[session_id] = body

            self.send_response(HTTPStatus.CREATED)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(b"{}")
            return

        # Relay fallback: POST /relay/<session_id>/<client_id>
        if len(parts) == 4 and parts[1] == "relay":
            _, _, session_id, client_id = parts

            length = int(self.headers.get("Content-Length", "0"))
            raw = self.rfile.read(length) if length > 0 else b""

            try:
                body = json.loads(raw.decode("utf-8")) if raw else {}
            except Exception:
                self.send_error(HTTPStatus.BAD_REQUEST, "Invalid JSON body")
                return

            to_id = body.get("to")
            if not isinstance(to_id, str) or not to_id:
                self.send_error(HTTPStatus.BAD_REQUEST, "Missing 'to' field")
                return

            data = body.get("data")

            session_queues = _RELAY_QUEUES.setdefault(session_id, {})
            queue = session_queues.setdefault(to_id, [])
            queue.append({"from": client_id, "data": data})

            self.send_response(HTTPStatus.CREATED)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(b"{}")
            return

        # Fallback to the original JSON APIs for manifests and state blobs.
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
            # Body is raw manifest JSON and is stored as-is.
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
            - POST /sessions/<session_id>             (body: manifest JSON)
            - GET  /sessions/<session_id>             (returns manifest JSON)
            - POST /state/<session_id>                (body: {"state": base64_string})
            - GET  /state/<session_id>                (returns same JSON)
            - POST /webrtc/<session_id>/<role>        (body: arbitrary JSON SDP/ICE blob)
            - GET  /webrtc/<session_id>/<role>        (returns last stored blob)
            - POST /relay/<session_id>/<client_id>    (body: {"to": "other_id", "data": ...})
            - GET  /relay/<session_id>/<client_id>    (returns and clears queued messages)

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
