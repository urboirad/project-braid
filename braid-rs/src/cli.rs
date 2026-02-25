use crate::emulator::{launch_emulator, DEFAULT_CORE};
use crate::hash::compute_rom_hash;
use crate::manifest::GameManifest;
use crate::nat::{negotiate_peer, run_nat_signaling_server};
use crate::session_link::SessionLink;
use crate::signaling::{get_manifest, get_state, post_manifest, post_state};
use clap::{Parser, Subcommand};
use reqwest::Client;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(
    name = "braid",
    about = "Project Braid - lightweight netplay wrapper for retro emulators (Rust prototype)",
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Host a new Braid session
    Host {
        /// Path to the ROM file
        rom: PathBuf,
        /// Override game title shown in manifest
        #[arg(long)]
        title: Option<String>,
        /// Libretro core name
        #[arg(long)]
        core: Option<String>,
        /// Rollback frame delay
        #[arg(long, default_value_t = 2)]
        frame_delay: i32,
        /// Directory where session manifests are stored locally
        #[arg(long, default_value = "./sessions")]
        session_dir: PathBuf,
        /// Override auto-generated session ID
        #[arg(long)]
        session_id: Option<String>,
        /// Base URL of signaling server, e.g. http://localhost:8080
        #[arg(long)]
        signal_url: Option<String>,
        /// Optional path to a small save-state file to upload via signaling
        #[arg(long)]
        state_file: Option<PathBuf>,
        /// Launch emulator as host after creating the session
        #[arg(long)]
        launch_emulator: bool,
        /// Emulator binary to launch (default: retroarch)
        #[arg(long, default_value = "retroarch")]
        emulator_bin: String,
        /// Print emulator command without executing it
        #[arg(long)]
        dry_run: bool,
        /// Optional NAT signaling server address (ip:port) for UDP hole punching
        #[arg(long)]
        nat_server: Option<String>,
    },

    /// Join an existing Braid session from a braid:// link
    Join {
        /// braid:// session link
        link: String,
        /// Path to local ROM for hash verification and launch
        #[arg(long)]
        rom: Option<PathBuf>,
        /// Launch emulator as peer after verifying manifest/ROM
        #[arg(long)]
        launch_emulator: bool,
        /// Emulator binary to launch (default: retroarch)
        #[arg(long, default_value = "retroarch")]
        emulator_bin: String,
        /// Host address to pass to emulator --connect (e.g. 12.34.56.78)
        #[arg(long)]
        connect_address: Option<String>,
        /// Attempt to auto-fetch a save-state blob via signaling before launch
        #[arg(long)]
        auto_state: bool,
        /// Where to write fetched save-state (default: <session_id>.state)
        #[arg(long)]
        state_output: Option<PathBuf>,
        /// Print emulator command without executing it
        #[arg(long)]
        dry_run: bool,
        /// Optional NAT signaling server address (ip:port) for UDP hole punching
        #[arg(long)]
        nat_server: Option<String>,
    },

    /// Run a minimal UDP-based NAT signaling server
    NatServer {
        /// Address to bind (ip:port), for example 0.0.0.0:40000
        #[arg(long, default_value = "0.0.0.0:40000")]
        bind: String,
    },

    /// Development helpers for pushing/pulling save-state blobs via signaling
    State {
        #[command(subcommand)]
        cmd: StateCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum StateCommands {
    /// Push a fake save-state file to signaling server
    Push {
        /// Session id to associate the state with
        session_id: String,
        /// Base URL of signaling server, e.g. http://localhost:8080
        signal_url: String,
        /// Path to a small binary state file to upload
        file: PathBuf,
    },
    /// Fetch a save-state blob from signaling server
    Pull {
        /// Session id to fetch state for
        session_id: String,
        /// Base URL of signaling server, e.g. http://localhost:8080
        signal_url: String,
    },
}

pub async fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Commands::Host {
            rom,
            title,
            core,
            frame_delay,
            session_dir,
            session_id,
            signal_url,
            state_file,
            launch_emulator: do_launch,
            emulator_bin,
            dry_run,
            nat_server,
        } => {
            run_host(
                rom,
                title,
                core,
                frame_delay,
                session_dir,
                session_id,
                signal_url,
                state_file,
                do_launch,
                emulator_bin,
                dry_run,
                nat_server,
            )
            .await
        }
        Commands::Join {
            link,
            rom,
            launch_emulator: do_launch,
            emulator_bin,
            connect_address,
            auto_state,
            state_output,
            dry_run,
            nat_server,
        } => {
            run_join(
                link,
                rom,
                do_launch,
                emulator_bin,
                connect_address,
                auto_state,
                state_output,
                dry_run,
                nat_server,
            )
            .await
        }
        Commands::NatServer { bind } => run_nat_signaling_server(&bind).await,
        Commands::State { cmd } => run_state(cmd).await,
    }
}

async fn run_state(cmd: StateCommands) -> Result<(), String> {
    let client = Client::new();

    match cmd {
        StateCommands::Push {
            session_id,
            signal_url,
            file,
        } => {
            let data = std::fs::read(&file)
                .map_err(|e| format!("failed to read state file: {e}"))?;
            post_state(&client, &signal_url, &session_id, &data)
                .await
                .map_err(|e| format!("failed to push state: {e}"))?;
            println!(
                "[braid-rs] pushed state blob for session {} ({} bytes)",
                session_id,
                data.len()
            );
        }
        StateCommands::Pull {
            session_id,
            signal_url,
        } => {
            let data = get_state(&client, &signal_url, &session_id)
                .await
                .map_err(|e| format!("failed to fetch state: {e}"))?;
            println!(
                "[braid-rs] fetched state blob for session {} ({} bytes)",
                session_id,
                data.len()
            );
        }
    }

    Ok(())
}

async fn run_host(
    rom: PathBuf,
    title: Option<String>,
    core: Option<String>,
    frame_delay: i32,
    session_dir: PathBuf,
    session_id: Option<String>,
    signal_url: Option<String>,
    state_file: Option<PathBuf>,
    do_launch: bool,
    emulator_bin: String,
    dry_run: bool,
    nat_server: Option<String>,
) -> Result<(), String> {
    let rom = rom
        .canonicalize()
        .map_err(|e| format!("ROM not found: {e}"))?;

    let rom_hash = compute_rom_hash(&rom).map_err(|e| format!("failed to hash ROM: {e}"))?;
    let game_title = title.unwrap_or_else(|| rom.file_stem().unwrap_or_default().to_string_lossy().to_string());

    let manifest = GameManifest {
        game_title,
        rom_hash,
        emulator_core: core.unwrap_or_else(|| DEFAULT_CORE.to_string()),
        sync_method: "rollback".to_string(),
        frame_delay,
    };

    let session_id = session_id.unwrap_or_else(|| Uuid::new_v4().to_string()[..12].to_string());

    std::fs::create_dir_all(&session_dir)
        .map_err(|e| format!("failed to create session dir: {e}"))?;
    let manifest_path = session_dir.join(format!("{}.json", session_id));
    let manifest_json = manifest
        .to_json()
        .map_err(|e| format!("failed to serialize manifest: {e}"))?;
    std::fs::write(&manifest_path, &manifest_json)
        .map_err(|e| format!("failed to write manifest: {e}"))?;

    let client = Client::new();
    let mut signal_url_effective: Option<String> = None;

    if let Some(ref url) = signal_url {
        if let Err(err) = post_manifest(&client, url, &session_id, &manifest).await {
            eprintln!("[braid-rs] warning: {err}");
        } else {
            signal_url_effective = Some(url.clone());

            // If a state file is provided, upload it via /state/<session_id>
            if let Some(ref state_path) = state_file {
                match std::fs::read(state_path) {
                    Ok(bytes) => {
                        if let Err(err) = post_state(&client, url, &session_id, &bytes).await {
                            eprintln!("[braid-rs] warning: failed to push state blob: {err}");
                        }
                    }
                    Err(err) => {
                        eprintln!("[braid-rs] warning: state file not readable: {err}");
                    }
                }
            }
        }
    }

    let link = SessionLink {
        session_id: session_id.clone(),
        signal_url: signal_url_effective.clone(),
        manifest_path: if signal_url_effective.is_some() {
            None
        } else {
            Some(manifest_path.to_string_lossy().to_string())
        },
    };

    println!("[braid-rs] Session created");
    println!("  Game:       {}", manifest.game_title);
    println!("  ROM hash:   {}", manifest.rom_hash);
    println!("  Core:       {}", manifest.emulator_core);
    println!("  Sync:       {} (delay={})", manifest.sync_method, manifest.frame_delay);
    println!();
    println!("Share this link with a peer:");
    println!("{}", link.to_uri().map_err(|e| e.to_string())?);

    let mut connect_addr: Option<String> = None;

    if let Some(server) = nat_server {
        if let Some(peer) = negotiate_peer(&server, &session_id).await? {
            println!("[braid-rs] NAT negotiation suggests peer address: {peer}");
            connect_addr = Some(peer.ip().to_string());
        } else {
            println!("[braid-rs] NAT negotiation did not find a peer yet.");
        }
    }

    if do_launch {
        launch_emulator(
            &emulator_bin,
            &manifest.emulator_core,
            &rom,
            "host",
            connect_addr.as_deref(),
            None,
            dry_run,
        )?;
    }

    Ok(())
}

async fn run_join(
    link_raw: String,
    rom: Option<PathBuf>,
    do_launch: bool,
    emulator_bin: String,
    connect_address: Option<String>,
    auto_state: bool,
    state_output: Option<PathBuf>,
    dry_run: bool,
    nat_server: Option<String>,
) -> Result<(), String> {
    let link = SessionLink::parse(&link_raw).map_err(|e| e.to_string())?;

    let client = Client::new();

    let manifest_json = if let Some(ref url) = link.signal_url {
        get_manifest(&client, url, &link.session_id).await?
    } else if let Some(ref manifest_path) = link.manifest_path {
        std::fs::read_to_string(manifest_path)
            .map_err(|e| format!("failed to read manifest: {e}"))?
    } else {
        return Err("invalid SessionLink: no signaling URL or manifest path".into());
    };

    let manifest = GameManifest::from_json(&manifest_json)
        .map_err(|e| format!("failed to parse manifest: {e}"))?;

    println!("[braid-rs] Joining session");
    println!("  Session ID: {}", link.session_id);
    println!("  Game:       {}", manifest.game_title);
    println!("  Expected hash: {}", manifest.rom_hash);
    println!("  Core:       {}", manifest.emulator_core);

    // Optional auto-fetch of a small save-state blob via signaling to
    // approximate an "Instant Join" path.
    if auto_state {
        if let Some(ref url) = link.signal_url {
            match get_state(&client, url, &link.session_id).await {
                Ok(data) => {
                    let out_path = state_output
                        .unwrap_or_else(|| PathBuf::from(format!("{}.state", link.session_id)));
                    if let Err(err) = std::fs::write(&out_path, &data) {
                        println!(
                            "[braid-rs] warning: error writing save-state to {}: {err}",
                            out_path.display()
                        );
                    } else {
                        println!("[braid-rs] Pulled save-state blob for session.");
                        println!("  File: {} ({} bytes)", out_path.display(), data.len());
                    }
                }
                Err(err) => {
                    println!("[braid-rs] warning: error fetching save-state: {err}");
                }
            }
        } else {
            println!(
                "[braid-rs] auto-state requested, but link does not carry a signaling URL."
            );
        }
    }

    let mut rom_path_verified: Option<PathBuf> = None;
    if let Some(rom_path) = rom {
        let rom_path = rom_path
            .canonicalize()
            .map_err(|e| format!("ROM not found: {e}"))?;
        let local_hash = compute_rom_hash(&rom_path)
            .map_err(|e| format!("failed to hash ROM: {e}"))?;
        if local_hash != manifest.rom_hash {
            println!("[braid-rs] ROM hash mismatch!");
            println!("  Expected: {}", manifest.rom_hash);
            println!("  Found:    {}", local_hash);
            println!("  ROM versions appear different; desync likely.");
            return Err("ROM hash mismatch".into());
        } else {
            println!("[braid-rs] ROM hash verified: OK");
            rom_path_verified = Some(rom_path);
        }
    } else {
        println!("[braid-rs] No ROM provided for verification.");
    }

    let mut effective_connect = connect_address;

    if effective_connect.is_none() {
        if let Some(server) = nat_server {
            if let Some(peer) = negotiate_peer(&server, &link.session_id).await? {
                println!("[braid-rs] NAT negotiation suggests host address: {peer}");
                effective_connect = Some(peer.ip().to_string());
            } else {
                println!("[braid-rs] NAT negotiation did not find a host yet.");
            }
        }
    }

    if do_launch {
        let rom_path = rom_path_verified
            .ok_or_else(|| "cannot launch emulator without verified --rom".to_string())?;

        let addr = effective_connect
            .ok_or_else(|| "--connect-address or --nat-server is required when launching the peer emulator".to_string())?;

        launch_emulator(
            &emulator_bin,
            &manifest.emulator_core,
            &rom_path,
            "peer",
            Some(&addr),
            None,
            dry_run,
        )?;
    }

    Ok(())
}
