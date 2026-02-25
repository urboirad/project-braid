use eframe::egui;
use std::net::TcpStream;
use std::time::{Duration, Instant};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Braid Netplay Dashboard",
        options,
        Box::new(|_cc| Ok(Box::new(BraidApp::new()))),
    )
}

struct BraidApp {
    host_rom: String,
    host_signal_url: String,
    host_nat_server: String,
    host_launch: bool,
    host_dry_run: bool,
    host_status: String,
    host_latency_ms: Option<f32>,

    join_link: String,
    join_rom: String,
    join_nat_server: String,
    join_launch: bool,
    join_dry_run: bool,
    join_status: String,
    join_latency_ms: Option<f32>,

    log: String,
    log_rx: Option<Receiver<String>>,
}

impl BraidApp {
    fn new() -> Self {
        Self {
            host_rom: String::new(),
            host_signal_url: String::new(),
            host_nat_server: String::new(),
            host_launch: false,
            host_dry_run: true,
            host_status: "Idle".to_string(),
            host_latency_ms: None,

            join_link: String::new(),
            join_rom: String::new(),
            join_nat_server: String::new(),
            join_launch: false,
            join_dry_run: true,
            join_status: "Idle".to_string(),
            join_latency_ms: None,

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

fn measure_tcp_latency(url_str: &str) -> Option<f32> {
    if url_str.trim().is_empty() {
        return None;
    }

    let parsed = url::Url::parse(url_str).ok()?;
    let host = parsed.host_str()?;
    let port = parsed.port_or_known_default()?;
    let addr = format!("{}:{}", host, port);

    let start = Instant::now();
    let timeout = Duration::from_secs(2);
    let stream = std::net::TcpStream::connect_timeout(&addr.parse().ok()?, timeout).ok()?;
    let _ = stream;
    let elapsed = start.elapsed();
    Some(elapsed.as_secs_f32() * 1000.0)
}

fn extract_signal_url_from_link(link: &str) -> Option<String> {
    if link.trim().is_empty() {
        return None;
    }

    let url = url::Url::parse(link).ok()?;
    let mut signal: Option<String> = None;
    for (k, v) in url.query_pairs() {
        if k == "signal" && !v.is_empty() {
            signal = Some(v.into_owned());
            break;
        }
    }
    signal
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
            ui.label("Tip: drag a ROM file onto this window to populate the host/join fields.");

            // Handle drag-and-drop of ROM files into the window. The first
            // dropped file populates the host ROM, the second the join ROM.
            let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
            for file in dropped_files {
                if let Some(path) = file.path {
                    let path_str = path.to_string_lossy().to_string();
                    if self.host_rom.is_empty() {
                        self.host_rom = path_str;
                    } else if self.join_rom.is_empty() {
                        self.join_rom = path_str;
                    }
                }
            }

            ui.horizontal(|ui| {
                ui.label("Host ROM:");
                ui.text_edit_singleline(&mut self.host_rom);
            });
            ui.horizontal(|ui| {
                ui.label("Signal URL:");
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

            ui.horizontal(|ui| {
                ui.label(format!("Status: {}", self.host_status));
                let latency_text = match self.host_latency_ms {
                    Some(ms) => format!("Latency: {:.1} ms", ms),
                    None => "Latency: N/A".to_string(),
                };
                ui.label(latency_text);
                if ui.button("Ping signaling").clicked() {
                    if let Some(ms) = measure_tcp_latency(&self.host_signal_url) {
                        self.host_latency_ms = Some(ms);
                        self.host_status = "Signaling reachable".to_string();
                    } else {
                        self.host_latency_ms = None;
                        self.host_status = "Signaling unreachable".to_string();
                    }
                }
            });

            if ui.button("Host session").clicked() {
                let mut args = Vec::<String>::new();
                args.push("host".to_string());
                if !self.host_rom.is_empty() {
                    args.push(self.host_rom.clone());
                }
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

            ui.horizontal(|ui| {
                ui.label(format!("Status: {}", self.join_status));
                let latency_text = match self.join_latency_ms {
                    Some(ms) => format!("Latency: {:.1} ms", ms),
                    None => "Latency: N/A".to_string(),
                };
                ui.label(latency_text);

                if ui.button("Ping signaling from link").clicked() {
                    if let Some(url) = extract_signal_url_from_link(&self.join_link) {
                        if let Some(ms) = measure_tcp_latency(&url) {
                            self.join_latency_ms = Some(ms);
                            self.join_status = "Signaling reachable".to_string();
                        } else {
                            self.join_latency_ms = None;
                            self.join_status = "Signaling unreachable".to_string();
                        }
                    } else {
                        self.join_status = "Invalid or missing braid:// link".to_string();
                        self.join_latency_ms = None;
                    }
                }
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
