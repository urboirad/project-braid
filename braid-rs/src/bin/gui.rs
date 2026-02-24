use eframe::egui;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Braid Netplay Dashboard",
        options,
        Box::new(|_cc| Box::new(BraidApp::new())),
    )
}

struct BraidApp {
    host_rom: String,
    host_session_dir: String,
    host_signal_url: String,
    host_nat_server: String,
    host_launch: bool,
    host_dry_run: bool,

    join_link: String,
    join_rom: String,
    join_nat_server: String,
    join_launch: bool,
    join_dry_run: bool,

    log: String,
    log_rx: Option<Receiver<String>>,
}

impl BraidApp {
    fn new() -> Self {
        Self {
            host_rom: String::new(),
            host_session_dir: "./sessions".to_string(),
            host_signal_url: String::new(),
            host_nat_server: String::new(),
            host_launch: false,
            host_dry_run: true,

            join_link: String::new(),
            join_rom: String::new(),
            join_nat_server: String::new(),
            join_launch: false,
            join_dry_run: true,

            log: String::new(),
            log_rx: None,
        }
    }

    fn spawn_cli(&mut self, args: Vec<String>) {
        let (tx, rx): (Sender<String>, Receiver<String>) = mpsc::channel();
        self.log_rx = Some(rx);

        // Clear log and echo command being run.
        self.log.clear();
        self.log.push_str(&format!("$ braid-rs {}\n", args.join(" ")));

        thread::spawn(move || {
            if let Err(err) = run_cli_command(args, tx.clone()) {
                let _ = tx.send(format!("[braid-rs-gui] error: {err}\n"));
            }
        });
    }
}

fn run_cli_command(args: Vec<String>, tx: Sender<String>) -> Result<(), String> {
    // Try to locate the CLI binary next to this executable, replacing
    // "braid-rs-gui" with "braid-rs" when possible.
    let mut exe = std::env::current_exe().map_err(|e| format!("current_exe failed: {e}"))?;
    if let Some(file_name) = exe.file_name().and_then(|s| s.to_str()) {
        if file_name.contains("braid-rs-gui") {
            exe.set_file_name(file_name.replace("braid-rs-gui", "braid-rs"));
        }
    }

    let mut child = Command::new(exe)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn braid-rs: {e}"))?;

    let mut readers = Vec::new();

    if let Some(stdout) = child.stdout.take() {
        let tx_clone = tx.clone();
        readers.push(thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            let reader = BufReader::new(stdout);
            for line in reader.lines().flatten() {
                let _ = tx_clone.send(format!("{}\n", line));
            }
        }));
    }

    if let Some(stderr) = child.stderr.take() {
        let tx_clone = tx.clone();
        readers.push(thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            let reader = BufReader::new(stderr);
            for line in reader.lines().flatten() {
                let _ = tx_clone.send(format!("{}\n", line));
            }
        }));
    }

    let status = child
        .wait()
        .map_err(|e| format!("failed to wait on braid-rs: {e}"))?;

    for handle in readers {
        let _ = handle.join();
    }

    if !status.success() {
        return Err(format!("braid-rs exited with status {status}"));
    }

    Ok(())
}

impl eframe::App for BraidApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Drain any new log lines from the CLI process.
        if let Some(rx) = &self.log_rx {
            while let Ok(line) = rx.try_recv() {
                self.log.push_str(&line);
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Braid Netplay Dashboard");
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Host ROM:");
                ui.text_edit_singleline(&mut self.host_rom);
            });
            ui.horizontal(|ui| {
                ui.label("Session dir:");
                ui.text_edit_singleline(&mut self.host_session_dir);
            });
            ui.horizontal(|ui| {
                ui.label("Signal URL (optional):");
                ui.text_edit_singleline(&mut self.host_signal_url);
            });
            ui.horizontal(|ui| {
                ui.label("NAT server (ip:port, optional):");
                ui.text_edit_singleline(&mut self.host_nat_server);
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.host_launch, "Launch emulator");
                ui.checkbox(&mut self.host_dry_run, "Dry run");
            });

            if ui.button("Host session").clicked() {
                let mut args = Vec::<String>::new();
                args.push("host".to_string());
                if !self.host_rom.is_empty() {
                    args.push(self.host_rom.clone());
                }

                args.push("--session-dir".to_string());
                args.push(self.host_session_dir.clone());

                if !self.host_signal_url.is_empty() {
                    args.push("--signal-url".to_string());
                    args.push(self.host_signal_url.clone());
                }

                if !self.host_nat_server.is_empty() {
                    args.push("--nat-server".to_string());
                    args.push(self.host_nat_server.clone());
                }

                if self.host_launch {
                    args.push("--launch-emulator".to_string());
                }
                if self.host_dry_run {
                    args.push("--dry-run".to_string());
                }

                self.spawn_cli(args);
            }

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Join link:");
                ui.text_edit_singleline(&mut self.join_link);
            });
            ui.horizontal(|ui| {
                ui.label("Join ROM (optional):");
                ui.text_edit_singleline(&mut self.join_rom);
            });
            ui.horizontal(|ui| {
                ui.label("NAT server (ip:port, optional):");
                ui.text_edit_singleline(&mut self.join_nat_server);
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.join_launch, "Launch emulator");
                ui.checkbox(&mut self.join_dry_run, "Dry run");
            });

            if ui.button("Join session").clicked() {
                let mut args = Vec::<String>::new();
                args.push("join".to_string());
                if !self.join_link.is_empty() {
                    args.push(self.join_link.clone());
                }

                if !self.join_rom.is_empty() {
                    args.push("--rom".to_string());
                    args.push(self.join_rom.clone());
                }

                if !self.join_nat_server.is_empty() {
                    args.push("--nat-server".to_string());
                    args.push(self.join_nat_server.clone());
                }

                if self.join_launch {
                    args.push("--launch-emulator".to_string());
                }
                if self.join_dry_run {
                    args.push("--dry-run".to_string());
                }

                self.spawn_cli(args);
            }

            ui.separator();
            ui.label("CLI log:");
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.monospace(&self.log);
                });
        });
    }
}
