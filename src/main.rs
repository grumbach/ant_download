mod server;

use server::Server;
use server::{DEFAULT_ENVIRONMENT, ENVIRONMENTS};

use eframe::egui;
use std::collections::HashMap;
use std::io::Write;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
struct DownloadStatus {
    is_downloading: bool,
    error_message: Option<String>,
    total_bytes_received: usize,
    chunks_received: usize,
}

#[derive(Debug, Clone)]
struct DownloadItem {
    address: String,
    status: DownloadStatus,
    save_path: Option<std::path::PathBuf>,
    file_size: usize,
    created_at: std::time::SystemTime,
}

enum DownloadEvent {
    Started { id: String },
    ChunkReceived { id: String, size: usize },
    Completed { id: String },
    Error { id: String, error: String },
}

struct AntDownloadApp {
    address_input: String,
    selected_env: String,
    is_connecting: bool,
    downloads: HashMap<String, DownloadItem>,
    download_receiver: mpsc::UnboundedReceiver<DownloadEvent>,
    download_sender: mpsc::UnboundedSender<DownloadEvent>,
}

impl Default for AntDownloadApp {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            address_input: String::new(),
            selected_env: DEFAULT_ENVIRONMENT.to_string(),
            is_connecting: false,
            downloads: HashMap::new(),
            download_receiver: rx,
            download_sender: tx,
        }
    }
}

impl eframe::App for AntDownloadApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request continuous repaints while any downloads are active
        if self.is_connecting || self.downloads.values().any(|d| d.status.is_downloading) {
            ctx.request_repaint();
        }

        // Process download events
        while let Ok(event) = self.download_receiver.try_recv() {
            match event {
                DownloadEvent::Started { id } => {
                    if let Some(download) = self.downloads.get_mut(&id) {
                        download.status.is_downloading = true;
                        download.status.error_message = None;
                    }
                    self.is_connecting = false;
                }
                DownloadEvent::ChunkReceived { id, size } => {
                    if let Some(download) = self.downloads.get_mut(&id) {
                        download.status.chunks_received += 1;
                        download.status.total_bytes_received += size;
                        download.file_size += size;
                    }
                }
                DownloadEvent::Completed { id } => {
                    if let Some(download) = self.downloads.get_mut(&id) {
                        download.status.is_downloading = false;
                    }
                }
                DownloadEvent::Error { id, error } => {
                    if let Some(download) = self.downloads.get_mut(&id) {
                        download.status.is_downloading = false;
                        download.status.error_message = Some(error);
                    }
                    self.is_connecting = false;
                }
            }
        }

        // Main UI
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.add_space(10.0);

                // Top bar with address input and download button
                ui.horizontal(|ui| {
                    ui.label("Address:");
                    let response = ui.add_sized(
                        [400.0, 22.0],
                        egui::TextEdit::singleline(&mut self.address_input)
                            .hint_text("Enter file address..."),
                    );

                    // Environment selector
                    egui::ComboBox::from_label("🌐")
                        .selected_text(&self.selected_env)
                        .show_ui(ui, |ui| {
                            for env in ENVIRONMENTS {
                                ui.selectable_value(&mut self.selected_env, env.to_string(), env);
                            }
                        });

                    let download_enabled =
                        !self.address_input.trim().is_empty() && !self.is_connecting;
                    ui.add_enabled_ui(download_enabled, |ui| {
                        if ui.button("Download").clicked() {
                            self.start_download();
                        }
                    });

                    // Status indicator
                    if self.is_connecting {
                        ui.add(egui::Spinner::new().size(12.0));
                        ui.label(
                            egui::RichText::new("Starting...")
                                .size(10.0)
                                .color(egui::Color32::YELLOW),
                        );
                    }

                    // Auto-focus on startup
                    if self.address_input.is_empty() {
                        response.request_focus();
                    }
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(5.0);

                // Downloads list
                self.show_downloads_list(ui);
            });
        });
    }
}

impl AntDownloadApp {
    fn start_download(&mut self) {
        let address = self.address_input.trim().to_string();
        if address.is_empty() {
            return;
        }

        let save_path = match rfd::FileDialog::new()
            .set_title("Save Downloaded File As")
            .save_file()
        {
            Some(path) => path,
            None => return, // User cancelled
        };

        // Generate unique download ID
        let download_id = format!(
            "{}_{}",
            address.chars().take(8).collect::<String>(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
                % 10000
        );

        // Create download item
        let download_item = DownloadItem {
            address: address.clone(),
            status: DownloadStatus {
                is_downloading: false,
                error_message: None,
                total_bytes_received: 0,
                chunks_received: 0,
            },
            save_path: Some(save_path.clone()),
            file_size: 0,
            created_at: std::time::SystemTime::now(),
        };

        self.downloads.insert(download_id.clone(), download_item);
        self.is_connecting = true;

        // Start download task
        let env = self.selected_env.clone();
        let tx = self.download_sender.clone();

        tokio::spawn(async move {
            // Initialize server
            match Server::new(&env).await {
                Ok(server) => {
                    let _ = tx.send(DownloadEvent::Started {
                        id: download_id.clone(),
                    });

                    // Create/open save file directly
                    match std::fs::File::create(&save_path) {
                        Ok(mut file) => {
                            // Start downloading
                            match server.stream_data(&address).await {
                                Ok(stream) => {
                                    for chunk_result in stream {
                                        match chunk_result {
                                            Ok(chunk) => {
                                                // Write chunk directly to save file
                                                if let Err(e) = file.write_all(&chunk) {
                                                    let _ = tx.send(DownloadEvent::Error {
                                                        id: download_id.clone(),
                                                        error: format!("Failed to write file: {e}"),
                                                    });
                                                    return;
                                                }

                                                if tx
                                                    .send(DownloadEvent::ChunkReceived {
                                                        id: download_id.clone(),
                                                        size: chunk.len(),
                                                    })
                                                    .is_err()
                                                {
                                                    break;
                                                }
                                            }
                                            Err(error) => {
                                                let _ = tx.send(DownloadEvent::Error {
                                                    id: download_id.clone(),
                                                    error,
                                                });
                                                return;
                                            }
                                        }
                                    }

                                    // Flush and complete
                                    if let Err(e) = file.flush() {
                                        let _ = tx.send(DownloadEvent::Error {
                                            id: download_id.clone(),
                                            error: format!("Failed to flush file: {e}"),
                                        });
                                        return;
                                    }

                                    let _ = tx.send(DownloadEvent::Completed { id: download_id });
                                }
                                Err(error) => {
                                    let _ = tx.send(DownloadEvent::Error {
                                        id: download_id,
                                        error,
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(DownloadEvent::Error {
                                id: download_id,
                                error: format!("Failed to create save file: {e}"),
                            });
                        }
                    }
                }
                Err(error) => {
                    let _ = tx.send(DownloadEvent::Error {
                        id: download_id,
                        error,
                    });
                }
            }
        });

        // Clear input for next download
        self.address_input.clear();
    }

    fn show_downloads_list(&mut self, ui: &mut egui::Ui) {
        if self.downloads.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                ui.label(
                    egui::RichText::new("📥")
                        .color(egui::Color32::GRAY)
                        .size(60.0),
                );
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("Ant Download")
                        .color(egui::Color32::GRAY)
                        .size(24.0),
                );
                ui.add_space(5.0);
                ui.label(
                    egui::RichText::new("Enter a file address above to start downloading")
                        .color(egui::Color32::DARK_GRAY)
                        .size(14.0),
                );
            });
            return;
        }

        // Sort downloads by creation time (newest first)
        let mut sorted_downloads: Vec<_> = self.downloads.iter().collect();
        sorted_downloads.sort_by(|a, b| b.1.created_at.cmp(&a.1.created_at));

        egui::ScrollArea::vertical()
            .max_height(ui.available_height() - 20.0)
            .show(ui, |ui| {
                for (_, download) in sorted_downloads {
                    self.show_download_item(ui, download);
                    ui.add_space(5.0);
                }
            });
    }

    fn show_download_item(&self, ui: &mut egui::Ui, download: &DownloadItem) {
        let frame = egui::Frame::default()
            .fill(egui::Color32::from_rgb(40, 40, 45))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 65)))
            .rounding(egui::Rounding::same(6.0))
            .inner_margin(egui::Margin::same(12.0));

        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                // Status indicator
                if download.status.is_downloading {
                    ui.add(egui::Spinner::new().size(16.0));
                } else if download.status.error_message.is_some() {
                    ui.label(egui::RichText::new("❌").size(16.0));
                } else if download.file_size > 0 {
                    ui.label(egui::RichText::new("✅").size(16.0));
                } else {
                    ui.label(egui::RichText::new("⏸").size(16.0));
                }

                ui.add_space(8.0);

                // Download info
                ui.vertical(|ui| {
                    // Filename and address
                    let filename = download.save_path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    
                    let display_address = download.address.clone();
                    
                    let display_text = format!("{} - {}", filename, display_address);
                    ui.label(
                        egui::RichText::new(display_text)
                            .color(egui::Color32::WHITE)
                            .size(13.0),
                    );

                    ui.add_space(2.0);

                    // Status and size
                    if let Some(error) = &download.status.error_message {
                        ui.label(
                            egui::RichText::new(format!("Error: {error}"))
                                .color(egui::Color32::LIGHT_RED)
                                .size(11.0),
                        );
                    } else if download.status.is_downloading {
                        ui.label(
                            egui::RichText::new(format!(
                                "Downloading... {}",
                                self.format_file_size(download.file_size)
                            ))
                            .color(egui::Color32::YELLOW)
                            .size(11.0),
                        );
                    } else if download.file_size > 0 {
                        ui.label(
                            egui::RichText::new(format!(
                                "Completed - {}",
                                self.format_file_size(download.file_size)
                            ))
                            .color(egui::Color32::LIGHT_GREEN)
                            .size(11.0),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new("Waiting...")
                                .color(egui::Color32::GRAY)
                                .size(11.0),
                        );
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Action buttons
                    let file_ready = download.file_size > 0
                        && download
                            .save_path
                            .as_ref()
                            .map(|p| p.exists())
                            .unwrap_or(false);

                    // Play button (only for media files)
                    if file_ready && self.is_media_file(download) {
                        if ui.small_button("▶ Play").clicked() {
                            self.play_download(download);
                        }
                    }
                });
            });
        });
    }

    fn is_media_file(&self, download: &DownloadItem) -> bool {
        if let Some(save_path) = &download.save_path {
            if let Some(extension) = save_path.extension().and_then(|e| e.to_str()) {
                let ext = extension.to_lowercase();
                matches!(
                    ext.as_str(),
                    "mp4" | "mov" | "avi" | "mkv" | "webm" | "mp3" | "wav" | "flac" | "ogg" | "m4a"
                )
            } else {
                false
            }
        } else {
            false
        }
    }

    fn play_download(&self, download: &DownloadItem) {
        if let Some(save_file) = &download.save_path {
            if save_file.exists() && download.file_size > 0 {
                #[cfg(target_os = "macos")]
                {
                    let _ = std::process::Command::new("open").arg(&save_file).spawn();
                }

                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "start", "", &save_file.to_string_lossy()])
                        .spawn();
                }

                #[cfg(target_os = "linux")]
                {
                    let _ = std::process::Command::new("xdg-open")
                        .arg(&save_file)
                        .spawn();
                }
            }
        }
    }

    fn format_file_size(&self, bytes: usize) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
        let mut size = bytes as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{} {}", bytes, UNITS[unit_index])
        } else {
            format!("{:.1} {}", size, UNITS[unit_index])
        }
    }
}

#[tokio::main]
async fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Ant Download")
            .with_inner_size([900.0, 700.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Ant Download",
        options,
        Box::new(|_cc| Box::new(AntDownloadApp::default())),
    )
}
