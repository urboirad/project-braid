from __future__ import annotations

from dataclasses import dataclass, asdict
from hashlib import sha1
from pathlib import Path
from typing import Any, Dict
import json


@dataclass
class GameManifest:
    game_title: str
    rom_hash: str
    emulator_core: str
    sync_method: str = "rollback"
    frame_delay: int = 2

    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), separators=(",", ":"))

    @classmethod
    def from_json(cls, data: str) -> "GameManifest":
        raw = json.loads(data)
        return cls(**raw)


def compute_rom_hash(path: Path) -> str:
    """Compute a stable SHA1 hash for ROM identity matching.

    This is used only for identity; no ROM bytes are shared by Braid itself.
    """
    h = sha1()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            h.update(chunk)
    return h.hexdigest()
