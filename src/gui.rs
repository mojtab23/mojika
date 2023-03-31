use std::collections::HashMap;
use std::fmt::{Display, Formatter};
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
                chat_text: String::new(),
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
    watch_peers: Receiver<HashMap<String, Peer>>,
    chat_text: String,
}

impl eframe::App for AppUi {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("header")
            .resizable(false)
            .exact_height(40.0)
            .show(ctx, |ui| {
                ui.heading(&self.title);
            });

        egui::SidePanel::left("peers_list")
            .resizable(false)
            .exact_width(240.0)
            .show(ctx, |ui| self.show_discoverd_peers(ui));

        egui::CentralPanel::default().show(ctx, |ui| {
            self.show_selected_peer(ui);
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
            for (_, peer) in peers.iter() {
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
        let selected_peer = &self.selected_peer_id;
        match selected_peer {
            None => {
                ui.label("No peer is selected.");
            }
            Some(selected_peer) => {
                ui.label(format!("Peer Id: {selected_peer}"));
                self.show_chat(ui);
            }
        }
    }

    fn show_chat(&mut self, ui: &mut Ui) {
        ui.group(|ui| {
            ui.heading("Chat");
            let peers = self.watch_peers.borrow().clone();
            let option = self.selected_peer_id.clone();
            match option {
                None => {
                    ui.label("nothing to show!");
                }
                Some(peer_id) => {
                    let peer_op = peers
                        .iter()
                        .find(|&(id, _)| id == &peer_id)
                        .map(|(_id, p)| p.clone());
                    self.chat_ui(ui, peer_op, &peer_id);
                }
            }
        });
    }

    fn chat_ui(&mut self, ui: &mut Ui, peer_op: Option<Peer>, peer_id: &str) {
        match peer_op {
            None => {
                ui.label("error finding the peer content!");
            }
            Some(p) => {
                if p.chat.messages.is_empty() {
                    ui.label("nothing to show!");
                } else {
                    for message in p.chat.messages.iter() {
                        let name = if message.sender == self.app.self_peer.id {
                            "Me"
                        } else {
                            &p.name
                        };
                        ui.label(format!("{name}: {}", message.content));
                    }
                }
            }
        }
        ui.horizontal(|ui| {
            let response = ui.add(egui::TextEdit::singleline(&mut self.chat_text));
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.send_chat(peer_id);
            }
            if ui.button("SEND").clicked() && !self.chat_text.is_empty() {
                self.send_chat(peer_id);
            }
            response.request_focus();
        });
    }

    fn send_chat(&mut self, peer_id: &str) {
        self.app
            .send_chat(peer_id, &self.app.self_peer.id, self.chat_text.clone());
        self.chat_text.clear();
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
