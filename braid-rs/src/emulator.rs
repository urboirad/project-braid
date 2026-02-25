use std::path::Path;
use std::process::Command;

pub const DEFAULT_CORE: &str = "snes9x_libretro";

pub fn build_retroarch_command(
    emulator_bin: &str,
    core: &str,
    rom_path: &Path,
    role: &str,
    connect_address: Option<&str>,
    extra_args: Option<&[String]>,
) -> Result<Vec<String>, String> {
    let mut cmd: Vec<String> = vec![emulator_bin.to_string(), "-L".into(), core.to_string()];

    match role {
        "host" => {
            cmd.push("--host".into());
        }
        "peer" => {
            let addr = connect_address.ok_or_else(|| "connect_address is required for peer role".to_string())?;
            cmd.push("--connect".into());
            cmd.push(addr.to_string());
        }
        other => {
            return Err(format!("unknown role: {other}"));
        }
    }

    if let Some(extra) = extra_args {
        cmd.extend(extra.iter().cloned());
    }

    cmd.push(rom_path.display().to_string());
    Ok(cmd)
}

pub fn launch_emulator(
    emulator_bin: &str,
    core: &str,
    rom_path: &Path,
    role: &str,
    connect_address: Option<&str>,
    extra_args: Option<&[String]>,
    dry_run: bool,
) -> Result<(), String> {
    let cmd = build_retroarch_command(
        emulator_bin,
        core,
        rom_path,
        role,
        connect_address,
        extra_args,
    )?;

    eprintln!("[braid-rs] emulator command:");
    eprintln!("  {}", cmd.join(" "));

    if dry_run {
        eprintln!("[braid-rs] (dry run) not executing emulator.");
        return Ok(());
    }

    let status = Command::new(&cmd[0])
        .args(&cmd[1..])
        .spawn()
        .map_err(|e| format!("failed to launch emulator: {e}"))?;

    let _ = status;
    Ok(())
}
