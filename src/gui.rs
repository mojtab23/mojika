use eframe::egui;
use egui::Ui;
use log::debug;
use tokio::sync::watch::Receiver;

pub fn new_gui(server_mode: bool, peer_discovery: Receiver<String>) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(320.0, 240.0)),
        ..Default::default()
    };
    let mode = if server_mode {
        "server mode"
    } else {
        "client mode"
    };
    let title = format!("Mojika Share ({mode})");

    eframe::run_native(
        "Mojika",
        options,
        Box::new(|_cc| {
            Box::new(MyApp {
                title,
                peer_discovery,
            })
        }),
    )
}

struct MyApp {
    title: String,
    peer_discovery: Receiver<String>,
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(&self.title);

            self.show_peers(ui);
        });
    }
}

impl MyApp {
    fn show_peers(&mut self, ui: &mut Ui) {
        let x = self.peer_discovery.borrow();
        if x.is_empty() {
            ui.label("Searching for Peer");
            ui.spinner();
        } else {
            ui.horizontal(|ui| {
                let peer = format!("Found Peer '{}'.", *x);
                ui.label(&peer);
                if ui.button("CONNECT").clicked() {
                    debug!("Connect...")
                }
            });
        }
    }
}
