mod cli;
mod hash;
mod manifest;
mod session_link;
mod emulator;
mod signaling;
mod nat;
mod gui;

use clap::Parser;

#[tokio::main]
async fn main() {
    let cli = cli::Cli::parse();
    if let Err(err) = cli::run(cli).await {
        eprintln!("[braid-rs] error: {err}");
        std::process::exit(1);
    }
}
