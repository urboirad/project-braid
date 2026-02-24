from __future__ import annotations

import shutil
import subprocess
from pathlib import Path
from typing import List


def build_retroarch_command(
    emulator_bin: str,
    core: str,
    rom_path: Path,
    role: str,
    connect_address: str | None = None,
    extra_args: list[str] | None = None,
) -> list[str]:
    """Build a RetroArch command approximating the design document.

    Note: Real RetroArch netplay flags differ by version; this is a conceptual
    stub that makes it easy to see and test how Braid would invoke it.
    """

    cmd: List[str] = [emulator_bin, "-L", core]

    if role == "host":
        # In a real implementation this would likely use --host / --netplay-host.
        cmd.append("--host")
    elif role == "peer":
        if not connect_address:
            raise ValueError("connect_address is required for peer role")
        cmd.extend(["--connect", connect_address])
    else:
        raise ValueError(f"Unknown role: {role}")

    if extra_args:
        cmd.extend(extra_args)

    cmd.append(str(rom_path))
    return cmd


def launch_emulator(
    emulator_bin: str,
    core: str,
    rom_path: Path,
    role: str,
    connect_address: str | None = None,
    extra_args: list[str] | None = None,
    dry_run: bool = False,
) -> None:
        """Launch RetroArch (or another emulator) if available.

        - If `dry_run` is True, only the command is printed.
        - If the emulator binary is not found on PATH, the command is printed and
            execution is skipped.
        """

    cmd = build_retroarch_command(
        emulator_bin=emulator_bin,
        core=core,
        rom_path=rom_path,
        role=role,
        connect_address=connect_address,
        extra_args=extra_args,
    )

    print("[braid] Emulator command:")
    print("  ", " ".join(cmd))

    if dry_run:
        print("[braid] (dry run) Not executing emulator.")
        return

    if shutil.which(emulator_bin) is None:
        print(f"[braid] Emulator binary '{emulator_bin}' not found on PATH.")
        print("        Install RetroArch or specify --emulator-bin, or run with --dry-run.")
        return

    try:
        subprocess.Popen(cmd)
        print("[braid] Emulator launched (background process).")
    except Exception as exc:  # pragma: no cover - environment dependent
        print(f"[braid] Failed to launch emulator: {exc}")
