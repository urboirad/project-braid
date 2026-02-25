#!/usr/bin/env bash
set -euo pipefail

# Simple "happy path" demo for Project Braid.
#
# Usage:
#   ./demo_happy_path.sh /absolute/path/to/game.rom
#
# This will:
#   - start the signaling server
#   - host a session for the ROM
#   - print the braid:// link to join
#
# You still need to manually run the join command (on this machine or another)
# using the printed link.

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 /absolute/path/to/game.rom" >&2
  exit 1
fi

ROM_PATH="$1"

if [[ ! -f "$ROM_PATH" ]]; then
  echo "[demo] ROM not found: $ROM_PATH" >&2
  exit 1
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

# Start signaling server in the background (Python prototype, used by Rust CLI)
python signaling_server.py &
SIGNAL_PID=$!

cleanup() {
  if kill -0 "$SIGNAL_PID" 2>/dev/null; then
    kill "$SIGNAL_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

sleep 1

echo "[demo] Hosting session for ROM: $ROM_PATH"

echo
cd "$ROOT_DIR/braid-rs"
cargo run --bin braid-rs -- host "$ROM_PATH" \
  --session-dir ../sessions \
  --signal-url http://localhost:8080 \
  --dry-run

echo
echo "[demo] Use the printed braid:// link on the joining machine, e.g.:"
echo "  cd braid-rs && cargo run --bin braid-rs -- join \"braid://<session_id>?signal=http%3A%2F%2Flocalhost%3A8080\" --rom '$ROM_PATH' --dry-run"
