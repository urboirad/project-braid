from __future__ import annotations

import argparse
import os
import sys
import textwrap
from pathlib import Path
from uuid import uuid4

from .manifest import GameManifest, compute_rom_hash
from .session_link import SessionLink


DEFAULT_CORE = "snes9x_libretro"


def _default_title_from_path(path: Path) -> str:
    return path.stem


def cmd_host(args: argparse.Namespace) -> int:
    rom_path = Path(args.rom).expanduser().resolve()
    if not rom_path.is_file():
        print(f"[braid] ROM not found: {rom_path}", file=sys.stderr)
        return 1

    rom_hash = compute_rom_hash(rom_path)
    game_title = args.title or _default_title_from_path(rom_path)

    manifest = GameManifest(
        game_title=game_title,
        rom_hash=rom_hash,
        emulator_core=args.core or DEFAULT_CORE,
        sync_method="rollback",
        frame_delay=args.frame_delay,
    )

    session_id = args.session_id or uuid4().hex[:12]
    session_dir = Path(args.session_dir).expanduser().resolve()
    session_dir.mkdir(parents=True, exist_ok=True)
    manifest_path = session_dir / f"{session_id}.json"
    manifest_path.write_text(manifest.to_json(), encoding="utf-8")

    link = SessionLink(session_id=session_id, manifest_path=str(manifest_path))

    print("[braid] Session created")
    print(f"  Game:       {manifest.game_title}")
    print(f"  ROM hash:   {manifest.rom_hash}")
    print(f"  Core:       {manifest.emulator_core}")
    print(f"  Sync:       {manifest.sync_method} (delay={manifest.frame_delay})")
    print()
    print("Share this link with your friend:")
    print(link.to_uri())

    # Stub: in the real implementation we'd also spin up signaling / netcode
    # here and eventually launch the emulator with --host / --connect.

    return 0


def cmd_join(args: argparse.Namespace) -> int:
    try:
        link = SessionLink.parse(args.link)
    except ValueError as exc:
        print(f"[braid] Invalid braid link: {exc}", file=sys.stderr)
        return 1

    manifest_path = Path(link.manifest_path).expanduser().resolve()
    if not manifest_path.is_file():
        print(f"[braid] Manifest not found: {manifest_path}", file=sys.stderr)
        print("Hint: for this prototype, the manifest must be accessible on your filesystem.")
        return 1

    from .manifest import GameManifest  # local import to avoid cycles

    manifest = GameManifest.from_json(manifest_path.read_text(encoding="utf-8"))

    print("[braid] Joining session")
    print(f"  Session ID: {link.session_id}")
    print(f"  Game:       {manifest.game_title}")
    print(f"  Expected hash: {manifest.rom_hash}")
    print(f"  Core:       {manifest.emulator_core}")

    if args.rom:
        rom_path = Path(args.rom).expanduser().resolve()
        if not rom_path.is_file():
            print(f"[braid] Provided ROM not found: {rom_path}", file=sys.stderr)
            return 1
        local_hash = compute_rom_hash(rom_path)
        if local_hash != manifest.rom_hash:
            print("[braid] ROM hash mismatch!")
            print(f"  Expected: {manifest.rom_hash}")
            print(f"  Found:    {local_hash}")
            print("  Your ROM version is different. Desync likely.")
            return 1
        else:
            print("[braid] ROM hash verified: OK")
    else:
        print("[braid] No ROM provided for verification (prototype).")

    # Stub: Here we would contact a signaling server, perform NAT traversal,
    # exchange save state, and finally launch the emulator subprocess.

    print()
    print("[braid] (Prototype) At this point, Braid would:")
    print("  - Perform NAT traversal / connect to relay if needed")
    print("  - Exchange real-time save state with the host")
    print("  - Launch the emulator with a --connect argument")

    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="braid",
        description="Project Braid - lightweight netplay wrapper for retro emulators (prototype)",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=textwrap.dedent(
            """\
            Examples:
              Host a session from a ROM file:
                braid host path/to/game.sfc

              Join using a braid:// link and verify ROM:
                braid join "braid://abc123?manifest=/tmp/abc123.json" --rom path/to/game.sfc
            """
        ),
    )

    sub = parser.add_subparsers(dest="command", required=True)

    p_host = sub.add_parser("host", help="Host a new Braid session")
    p_host.add_argument("rom", help="Path to the ROM file")
    p_host.add_argument("--title", help="Override game title shown in manifest")
    p_host.add_argument("--core", help="Libretro core name (default: snes9x_libretro)")
    p_host.add_argument("--frame-delay", type=int, default=2, help="Rollback frame delay")
    p_host.add_argument(
        "--session-dir",
        default=os.path.join(os.getcwd(), "sessions"),
        help="Directory where session manifests are stored",
    )
    p_host.add_argument("--session-id", help="Override auto-generated session ID")
    p_host.set_defaults(func=cmd_host)

    p_join = sub.add_parser("join", help="Join an existing Braid session from a braid:// link")
    p_join.add_argument("link", help="braid:// session link")
    p_join.add_argument("--rom", help="Path to local ROM for hash verification")
    p_join.set_defaults(func=cmd_join)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    func = getattr(args, "func", None)
    if func is None:
        parser.print_help()
        return 1
    return func(args)


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
