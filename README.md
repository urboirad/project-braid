# Project Braid

Project Braid is a lightweight wrapper and orchestration layer for retro emulators.

This repo currently contains a **Rust prototype CLI** that implements the early pieces of
the design:

- Generate a JSON manifest for a ROM (title, hash, core, sync settings).
- Push that manifest to a tiny HTTP signaling service keyed by `session_id`.
- Create a `braid://`-style session link that carries a `session_id` and signaling URL.
- Let a peer join using that link and verify they have the same ROM via hash.

Real-time netcode, save-state injection, NAT traversal, relay servers, and virtual
controller logic are intentionally **not** implemented yet; those will layer on
top of this shell.

## Requirements

- Rust 1.75+ with Cargo (tested on Linux).

The Rust crate uses a handful of common crates (clap, tokio, reqwest, egui, etc.) which
Cargo will fetch automatically.

## Quick start

From the project root:

```bash
chmod +x braid
./braid --help
```

You should see the `braid-rs` help with `host`, `join`, `nat-server`, and `state` subcommands.

### 1. Start the signaling server

In a separate terminal from the project root:

```bash
python signaling_server.py
```

You should see it listen on `http://0.0.0.0:8080` by default.

In addition to basic manifest and save-state storage, this server also exposes
lightweight endpoints for:

- **WebRTC-style signaling**: `POST/GET /webrtc/<session_id>/<role>` for
	exchanging arbitrary SDP/ICE blobs between a `host` and a `peer`.
- **Relay fallback**: `POST/GET /relay/<session_id>/<client_id>` for
	best-effort message passing when direct peer-to-peer signaling is not
	possible.

### 2. Host a session

```bash
./braid host /path/to/game.sfc \
	--signal-url http://localhost:8080
```

This will:

- Compute a SHA1 hash of the ROM for identity matching.
- Construct a manifest in memory and push it to the signaling server at
	`POST /sessions/<session_id>`.
- Print a `braid://<session_id>?signal=http%3A%2F%2Flocalhost%3A8080` link.

### 3. Join a session

On a peer machine (that can reach the signaling server):

```bash
./braid join "braid://<session_id>?signal=http%3A%2F%2Flocalhost%3A8080" \
	--rom /path/to/game.sfc
```

The join flow will:

- Parse the `braid://` link.
- Contact the signaling server at `GET /sessions/<session_id>` to fetch the
	manifest.
- Display expected ROM hash + core.
- Optionally hash the local ROM and compare; if hashes differ, it warns and
	exits.

### 4. Actually launch RetroArch

By default, `host` and `join` only print what they would do. To actually spawn
an emulator, pass `--launch-emulator` on each side:

```bash
./braid host /path/to/game.sfc \
	--signal-url http://localhost:8080 \
	--launch-emulator

./braid join "braid://<session_id>?signal=http%3A%2F%2Flocalhost%3A8080" \
	--rom /path/to/game.sfc \
	--connect-address 203.0.113.10 \
	--launch-emulator
```

Under the hood this wraps a RetroArch-style binary (configurable via
`--emulator-bin`, default `retroarch`) and constructs a command like:

- Host: `retroarch -L <core> --host /path/to/game.sfc`
- Peer: `retroarch -L <core> --connect <host_addr> /path/to/game.sfc`

You can also use `--nat-server ip:port` instead of `--connect-address` to let
the minimal UDP signaling server suggest a public peer/host address.

## Next steps / ideas

Planned directions that align with the original design:

- Add a minimal GUI (drag-and-drop host screen, join screen with latency and
	status indicators).

Contributions and experiments are welcome while this is still in the prototype
phase.
