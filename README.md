# Project Braid

Project Braid is a lightweight wrapper and orchestration layer for retro emulators.

This repo currently contains a **prototype CLI** that implements the early pieces of
the design:

- Generate a JSON manifest for a ROM (title, hash, core, sync settings).
- Create a `braid://`-style session link that encodes a session id + manifest path.
- Let a peer join using that link and verify they have the same ROM via hash.

Real-time netcode, save-state injection, NAT traversal, relay servers, and virtual
controller logic are intentionally **not** implemented yet; those will layer on
top of this shell.

## Requirements

- Python 3.10+ (tested with the system Python on Linux).

No external dependencies are required; everything uses the Python standard library.

## Quick start

From the project root:

```bash
python main.py -h
```

You should see the `braid` help with `host` and `join` subcommands.

### 1. Host a session

```bash
python main.py host /path/to/game.sfc \
	--session-dir ./sessions
```

This will:

- Compute a SHA1 hash of the ROM for identity matching.
- Write a manifest JSON file into `./sessions/<session_id>.json`.
- Print a `braid://<session_id>?manifest=/abs/path/to/manifest.json` link.

### 2. Join a session (prototype, local-only)

For now, the manifest path is a local file path, so the joiner needs access to
the same file (e.g., shared folder, or you copy it there manually):

```bash
python main.py join "braid://<session_id>?manifest=/abs/path/to/manifest.json" \
	--rom /path/to/game.sfc
```

The join flow will:

- Parse the `braid://` link.
- Load the manifest and display expected ROM hash + core.
- Optionally hash the local ROM and compare; if hashes differ, it warns and exits.

At this point the CLI prints what would happen next (NAT traversal, state sync,
emulator launch) but does not yet perform those actions.

## Next steps / ideas

Planned directions that align with the original design:

- Replace local manifest paths with a small signaling service keyed by
	`session_id`.
- Implement UDP hole punching / WebRTC-based signaling, with a relay fallback.
- Add a minimal GUI (drag-and-drop host screen, join screen with latency and
	status indicators).
- Wrap RetroArch (or similar) to actually spawn emulators with `--connect`.

Contributions and experiments are welcome while this is still in the prototype
phase.
