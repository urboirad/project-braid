from __future__ import annotations

import argparse
import os
import sys
import textwrap
from pathlib import Path
from urllib.parse import urljoin
from uuid import uuid4
import base64

from .manifest import GameManifest, compute_rom_hash
from .session_link import SessionLink
from .emulator import launch_emulator


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

    # Always persist a local copy of the manifest for debugging / tooling.
    session_dir = Path(args.session_dir).expanduser().resolve()
    session_dir.mkdir(parents=True, exist_ok=True)
    manifest_path = session_dir / f"{session_id}.json"
    manifest_json = manifest.to_json()
    manifest_path.write_text(manifest_json, encoding="utf-8")

    signal_url: str | None = None
    if args.signal_url:
        # POST manifest to signaling server: POST /sessions/<session_id>
        try:
            import urllib.request

            base = args.signal_url.rstrip("/") + "/"
            endpoint = urljoin(base, f"sessions/{session_id}")
            req = urllib.request.Request(
                endpoint,
                data=manifest_json.encode("utf-8"),
                headers={"Content-Type": "application/json"},
                method="POST",
            )
            with urllib.request.urlopen(req, timeout=5) as resp:  # noqa: S310
                if resp.status not in (200, 201):
                    print(
                        f"[braid] Warning: signaling server returned HTTP {resp.status}",
                        file=sys.stderr,
                    )
                else:
                    signal_url = args.signal_url
        except Exception as exc:  # pragma: no cover - depends on network env
            print(f"[braid] Warning: failed to contact signaling server: {exc}", file=sys.stderr)

    # If a signaling server and a state file are available, push a fake state blob
    # for this session so peers can auto-fetch it.
    if signal_url and getattr(args, "state_file", None):
        state_path = Path(args.state_file).expanduser().resolve()
        if not state_path.is_file():
            print(f"[braid] State file not found: {state_path}", file=sys.stderr)
        else:
            try:
                import urllib.request
                import json as _json

                data = state_path.read_bytes()
                state_b64 = base64.b64encode(data).decode("ascii")
                base = signal_url.rstrip("/") + "/"
                endpoint = urljoin(base, f"state/{session_id}")
                body = _json.dumps({"state": state_b64}).encode("utf-8")
                req = urllib.request.Request(
                    endpoint,
                    data=body,
                    headers={"Content-Type": "application/json"},
                    method="POST",
                )
                with urllib.request.urlopen(req, timeout=5) as resp:  # noqa: S310
                    if resp.status not in (200, 201):
                        print(
                            f"[braid] Warning: failed to push state blob (HTTP {resp.status})",
                            file=sys.stderr,
                        )
            except Exception as exc:  # pragma: no cover - network/env dependent
                print(f"[braid] Warning: error pushing state blob: {exc}", file=sys.stderr)

    link = SessionLink(
        session_id=session_id,
        signal_url=signal_url,
        manifest_path=None if signal_url else str(manifest_path),
    )

    print("[braid] Session created")
    print(f"  Game:       {manifest.game_title}")
    print(f"  ROM hash:   {manifest.rom_hash}")
    print(f"  Core:       {manifest.emulator_core}")
    print(f"  Sync:       {manifest.sync_method} (delay={manifest.frame_delay})")
    print()
    print("Share this link with your friend:")
    print(link.to_uri())

    # Optionally launch the emulator on the host side.
    if args.launch_emulator:
        launch_emulator(
            emulator_bin=args.emulator_bin,
            core=manifest.emulator_core,
            rom_path=rom_path,
            role="host",
            dry_run=args.dry_run,
        )

    return 0


def cmd_join(args: argparse.Namespace) -> int:
    try:
        link = SessionLink.parse(args.link)
    except ValueError as exc:
        print(f"[braid] Invalid braid link: {exc}", file=sys.stderr)
        return 1

    # Retrieve manifest either from signaling server or local filesystem.
    manifest_json: str | None = None

    if link.signal_url:
        try:
            import urllib.request

            base = link.signal_url.rstrip("/") + "/"
            endpoint = urljoin(base, f"sessions/{link.session_id}")
            with urllib.request.urlopen(endpoint, timeout=5) as resp:  # noqa: S310
                if resp.status != 200:
                    print(
                        f"[braid] Failed to fetch manifest from signaling server (HTTP {resp.status})",
                        file=sys.stderr,
                    )
                    return 1
                manifest_json = resp.read().decode("utf-8")
        except Exception as exc:  # pragma: no cover - depends on network env
            print(f"[braid] Error contacting signaling server: {exc}", file=sys.stderr)
            return 1
    elif link.manifest_path:
        manifest_path = Path(link.manifest_path).expanduser().resolve()
        if not manifest_path.is_file():
            print(f"[braid] Manifest not found: {manifest_path}", file=sys.stderr)
            print(
                "Hint: for this prototype, the manifest must be accessible on your filesystem.",
            )
            return 1
        manifest_json = manifest_path.read_text(encoding="utf-8")
    else:  # pragma: no cover - guarded by SessionLink
        print("[braid] Invalid SessionLink: no signaling URL or manifest path", file=sys.stderr)
        return 1

    from .manifest import GameManifest  # local import to avoid cycles

    manifest = GameManifest.from_json(manifest_json)

    print("[braid] Joining session")
    print(f"  Session ID: {link.session_id}")
    print(f"  Game:       {manifest.game_title}")
    print(f"  Expected hash: {manifest.rom_hash}")
    print(f"  Core:       {manifest.emulator_core}")

    # If requested and a signaling URL is available, try to pull a state blob for
    # this session before launching the emulator to approximate Instant Join.
    if getattr(args, "auto_state", False) and link.signal_url:
        try:
            import urllib.request
            import json as _json

            base = link.signal_url.rstrip("/") + "/"
            endpoint = urljoin(base, f"state/{link.session_id}")
            with urllib.request.urlopen(endpoint, timeout=5) as resp:  # noqa: S310
                if resp.status == 404:
                    print("[braid] No save-state found for this session (404).")
                elif resp.status != 200:
                    print(
                        f"[braid] Failed to fetch save-state (HTTP {resp.status})",
                        file=sys.stderr,
                    )
                else:
                    payload = _json.loads(resp.read().decode("utf-8"))
                    state_b64 = payload.get("state")
                    if isinstance(state_b64, str):
                        data = base64.b64decode(state_b64.encode("ascii"))
                        out_path = Path(args.state_output or f"{link.session_id}.state").resolve()
                        out_path.write_bytes(data)
                        print("[braid] Pulled save-state blob for session.")
                        print(f"  File: {out_path} ({len(data)} bytes)")
                    else:
                        print("[braid] Invalid save-state payload from server.", file=sys.stderr)
        except Exception as exc:  # pragma: no cover - network/env dependent
            print(f"[braid] Warning: error fetching save-state: {exc}", file=sys.stderr)

    rom_path: Path | None = None
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

    # Optionally launch the emulator on the peer side.
    if args.launch_emulator:
        if not rom_path:
            print("[braid] Cannot launch emulator without --rom", file=sys.stderr)
            return 1
        if not args.connect_address:
            print("[braid] --connect-address is required when launching the peer emulator", file=sys.stderr)
            return 1

        launch_emulator(
            emulator_bin=args.emulator_bin,
            core=manifest.emulator_core,
            rom_path=rom_path,
            role="peer",
            connect_address=args.connect_address,
            dry_run=args.dry_run,
        )

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
        help="Directory where session manifests are stored locally",
    )
    p_host.add_argument("--session-id", help="Override auto-generated session ID")
    p_host.add_argument(
        "--signal-url",
        help="Base URL of signaling server, e.g. http://localhost:8080",
    )
    p_host.add_argument(
        "--state-file",
        help="Optional path to a small save-state file to upload via signaling",
    )
    p_host.add_argument(
        "--launch-emulator",
        action="store_true",
        help="Launch emulator as host after creating the session",
    )
    p_host.add_argument(
        "--emulator-bin",
        default="retroarch",
        help="Emulator binary to launch (default: retroarch)",
    )
    p_host.add_argument(
        "--dry-run",
        action="store_true",
        help="Print emulator command without executing it",
    )
    p_host.set_defaults(func=cmd_host)

    p_join = sub.add_parser("join", help="Join an existing Braid session from a braid:// link")
    p_join.add_argument("link", help="braid:// session link")
    p_join.add_argument("--rom", help="Path to local ROM for hash verification and launch")
    p_join.add_argument(
        "--launch-emulator",
        action="store_true",
        help="Launch emulator as peer after verifying manifest/ROM",
    )
    p_join.add_argument(
        "--emulator-bin",
        default="retroarch",
        help="Emulator binary to launch (default: retroarch)",
    )
    p_join.add_argument(
        "--connect-address",
        help="Host address to pass to emulator --connect (e.g. 12.34.56.78)",
    )
    p_join.add_argument(
        "--auto-state",
        action="store_true",
        help="Attempt to auto-fetch a save-state blob via signaling before launch",
    )
    p_join.add_argument(
        "--state-output",
        help="Where to write fetched save-state (default: <session_id>.state)",
    )
    p_join.add_argument(
        "--dry-run",
        action="store_true",
        help="Print emulator command without executing it",
    )
    p_join.set_defaults(func=cmd_join)

    # Simple dev-only commands to push/pull a fake save-state blob through the
    # signaling server, approximating the "Instant Join" path described in the
    # design document.

    p_state = sub.add_parser("state", help="Experimental save-state helpers (prototype)")
    state_sub = p_state.add_subparsers(dest="state_cmd", required=True)

    p_push = state_sub.add_parser("push", help="Push a fake save-state file to signaling server")
    p_push.add_argument("session_id", help="Session id to associate the state with")
    p_push.add_argument("signal_url", help="Base URL of signaling server, e.g. http://localhost:8080")
    p_push.add_argument("file", help="Path to a small binary state file to upload")
    p_push.set_defaults(func=cmd_state_push)

    p_pull = state_sub.add_parser("pull", help="Fetch a save-state blob from signaling server")
    p_pull.add_argument("session_id", help="Session id to fetch state for")
    p_pull.add_argument("signal_url", help="Base URL of signaling server, e.g. http://localhost:8080")
    p_pull.set_defaults(func=cmd_state_pull)

    return parser


def cmd_state_push(args: argparse.Namespace) -> int:
    """Push a fake save-state binary through the signaling server.

    This is a development helper that lets you experiment with the
    state-exchange path (host → server → peer) using arbitrary small files.
    """

    state_path = Path(args.file).expanduser().resolve()
    if not state_path.is_file():
        print(f"[braid] State file not found: {state_path}", file=sys.stderr)
        return 1

    data = state_path.read_bytes()
    state_b64 = base64.b64encode(data).decode("ascii")

    try:
        import urllib.request
        import json as _json

        base = args.signal_url.rstrip("/") + "/"
        endpoint = urljoin(base, f"state/{args.session_id}")
        body = _json.dumps({"state": state_b64}).encode("utf-8")
        req = urllib.request.Request(
            endpoint,
            data=body,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(req, timeout=5) as resp:  # noqa: S310
            if resp.status not in (200, 201):
                print(f"[braid] Failed to push state (HTTP {resp.status})", file=sys.stderr)
                return 1
    except Exception as exc:  # pragma: no cover - network env dependent
        print(f"[braid] Error contacting signaling server: {exc}", file=sys.stderr)
        return 1

    print("[braid] Pushed save-state blob to signaling server.")
    print(f"  Session: {args.session_id}")
    print(f"  File:    {state_path}")
    print(f"  Size:    {len(data)} bytes (base64 length {len(state_b64)})")
    return 0


def cmd_state_pull(args: argparse.Namespace) -> int:
    """Fetch a fake save-state blob from the signaling server.

    This is a development helper that pulls the base64 state for a session and
    writes it out as a binary file (or prints its size if stdout only).
    """

    try:
        import urllib.request
        import json as _json

        base = args.signal_url.rstrip("/") + "/"
        endpoint = urljoin(base, f"state/{args.session_id}")
        with urllib.request.urlopen(endpoint, timeout=5) as resp:  # noqa: S310
            if resp.status != 200:
                print(
                    f"[braid] Failed to fetch state from signaling server (HTTP {resp.status})",
                    file=sys.stderr,
                )
                return 1
            payload = _json.loads(resp.read().decode("utf-8"))
    except Exception as exc:  # pragma: no cover - network env dependent
        print(f"[braid] Error contacting signaling server: {exc}", file=sys.stderr)
        return 1

    state_b64 = payload.get("state")
    if not isinstance(state_b64, str):
        print("[braid] Invalid state payload from server", file=sys.stderr)
        return 1

    data = base64.b64decode(state_b64.encode("ascii"))

    # The current prototype always writes to a file named <session_id>.state in CWD.
    out_path = Path(f"{args.session_id}.state").resolve()
    out_path.write_bytes(data)

    print("[braid] Pulled save-state blob from signaling server.")
    print(f"  Session: {args.session_id}")
    print(f"  File:    {out_path}")
    print(f"  Size:    {len(data)} bytes")
    return 0


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
