use std::sync::Arc;

use eframe::egui;
use egui::Ui;
use log::debug;
use tokio::sync::watch::Receiver;

use crate::app::peer::Peer;
use crate::app::App;

pub fn new_gui(app: Arc<App>) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(800.0, 600.0)),
        run_and_return: true,
        ..Default::default()
    };
    let mode = if app.server_mode {
        "server mode"
    } else {
        "client mode"
    };
    let title = format!("Mojika Share ({mode})");

    let watch_peers = app.watch_peers();

    let result = eframe::run_native(
        "Mojika",
        options,
        Box::new(|_cc| {
            Box::new(AppUi {
                app,
                title,
                watch_peers,
            })
        }),
    );
    debug!("after run native");
    result
}

struct AppUi {
    app: Arc<App>,
    title: String,
    watch_peers: Receiver<Vec<Peer>>,
}

impl eframe::App for AppUi {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(&self.title);

            self.show_peers(ui);
        });
    }
}

impl AppUi {
    fn show_peers(&mut self, ui: &mut Ui) {
        let peers = self.watch_peers.borrow();

        if peers.is_empty() {
            ui.label("Searching for Peer");
            ui.spinner();
        } else {
            for peer in peers.iter() {
                ui.horizontal(|ui| {
                    let short_id: String = peer.id.chars().take(4).collect();
                    let peer_text = format!("{} ({})", peer.name, short_id);
                    ui.label(&peer_text);
                    // if ui.button("SEND FILE").clicked() {
                    //     debug!("open file picker");
                    //     let file = rfd::FileDialog::new().pick_file();
                    //
                    //     if let Some(file) = file {
                    //         if let Some(filename) = file.file_name() {
                    //             let name = filename.to_str().unwrap_or_default();
                    //             info!("file:{name}");
                    //         }
                    //     } else {
                    //         debug!("No file selected.")
                    //     }
                    // }
                    if ui.button("CONNECT").clicked() {
                        debug!("Connect to {peer:?} clicked.");
                        self.app.connect_to_peer(&peer.id);
                    }
                });
            }
        }
    }
}
