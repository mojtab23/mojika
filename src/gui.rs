use std::fmt::{Display, Formatter};
use std::sync::Arc;

use eframe::egui;
use egui::{SelectableLabel, Ui};
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
    let name = app.self_peer.to_string();
    let title = format!("Mojika Share ({name})");

    let watch_peers = app.watch_peers();

    let result = eframe::run_native(
        "Mojika",
        options,
        Box::new(|_cc| {
            Box::new(AppUi {
                app,
                title,
                tab: Tab::Discovery,
                selected_peer_id: None,
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
    tab: Tab,
    selected_peer_id: Option<String>,
    watch_peers: Receiver<Vec<Peer>>,
}

impl eframe::App for AppUi {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(&self.title);

            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Discovery, Tab::Discovery.to_string());
                if ui
                    .add_enabled(
                        self.selected_peer_id.is_some(),
                        SelectableLabel::new(self.tab == Tab::PeerView, Tab::PeerView.to_string()),
                    )
                    .clicked()
                {
                    self.tab = Tab::PeerView;
                }
            });
            self.show_tab_content(ui);
        });
    }
}

impl AppUi {
    fn show_discoverd_peers(&mut self, ui: &mut Ui) {
        let peers = self.watch_peers.borrow();

        if peers.is_empty() {
            ui.label("Searching for Peer");
            ui.spinner();
        } else {
            for peer in peers.iter() {
                ui.horizontal(|ui| {
                    let peer_text = peer.to_string();
                    ui.label(&peer_text);
                    if ui.button("SHOW").clicked() {
                        debug!("Show {peer} clicked!");
                        self.selected_peer_id = Some(peer.id.clone());
                        self.tab = Tab::PeerView;
                    }
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

    fn show_selected_peer(&mut self, ui: &mut Ui) {
        ui.label(format!(
            "Peer Id: {}",
            self.selected_peer_id
                .clone()
                .unwrap_or("NONE!?".to_string())
        ));
    }
    fn show_tab_content(&mut self, ui: &mut Ui) {
        match self.tab {
            Tab::Discovery => self.show_discoverd_peers(ui),
            Tab::PeerView => self.show_selected_peer(ui),
        }
    }
}

#[derive(PartialEq)]
enum Tab {
    Discovery,
    PeerView,
}

impl Display for Tab {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Tab::Discovery => write!(f, "Discovery"),
            Tab::PeerView => write!(f, "View Peer"),
        }
    }
}
