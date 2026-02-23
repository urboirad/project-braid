from __future__ import annotations

from dataclasses import dataclass
from urllib.parse import urlparse, parse_qs, urlencode, urlunparse


BRAID_SCHEME = "braid"


@dataclass
class SessionLink:
    """Represents a braid:// session link.

    A link can carry either:

    - A local manifest path (prototype-only, for filesystem sharing), or
    - A signaling server base URL, which Braid uses to fetch the manifest.
    """

    session_id: str
    signal_url: str | None = None
    manifest_path: str | None = None

    @classmethod
    def parse(cls, uri: str) -> "SessionLink":
        parsed = urlparse(uri)
        if parsed.scheme != BRAID_SCHEME:
            raise ValueError(f"Unsupported scheme: {parsed.scheme}")
        session_id = parsed.netloc or parsed.path.lstrip("/")
        qs = parse_qs(parsed.query)

        signal_url = qs.get("signal", [""])[0] or None
        manifest_path = qs.get("manifest", [""])[0] or None

        if not signal_url and not manifest_path:
            raise ValueError("Missing both 'signal' and 'manifest' parameters in braid link")

        return cls(session_id=session_id, signal_url=signal_url, manifest_path=manifest_path)

    def to_uri(self) -> str:
        params: dict[str, str] = {}
        if self.signal_url:
            params["signal"] = self.signal_url
        if self.manifest_path:
            params["manifest"] = self.manifest_path

        if not params:
            raise ValueError("SessionLink must have either signal_url or manifest_path")

        query = urlencode(params)
        return urlunparse((BRAID_SCHEME, self.session_id, "", "", query, ""))
