from __future__ import annotations

from dataclasses import dataclass
from urllib.parse import urlparse, parse_qs, urlencode, urlunparse


BRAID_SCHEME = "braid"


@dataclass
class SessionLink:
    """Represents a braid:// session link.

    For the initial prototype, we embed the session_id and a reference to a
    local manifest file via query string. A real implementation would use
    a remote signaling/relay service instead.
    """

    session_id: str
    manifest_path: str

    @classmethod
    def parse(cls, uri: str) -> "SessionLink":
        parsed = urlparse(uri)
        if parsed.scheme != BRAID_SCHEME:
            raise ValueError(f"Unsupported scheme: {parsed.scheme}")
        session_id = parsed.netloc or parsed.path.lstrip("/")
        qs = parse_qs(parsed.query)
        manifest_path = qs.get("manifest", [""])[0]
        if not manifest_path:
            raise ValueError("Missing manifest parameter in braid link")
        return cls(session_id=session_id, manifest_path=manifest_path)

    def to_uri(self) -> str:
        query = urlencode({"manifest": self.manifest_path})
        return urlunparse((BRAID_SCHEME, self.session_id, "", "", query, ""))
