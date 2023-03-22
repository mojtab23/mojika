use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use anyhow::Result;
use log::{debug, error, info, warn};
use tokio::spawn;
use tokio::sync::broadcast::Receiver;
use tokio::sync::{mpsc, watch};
use tokio::time::sleep;

use crate::app::shutdown::ShutdownWatcher;
use crate::connect::{client, server};
use crate::discovery::Discovery;
use crate::gui;

pub struct App {}

impl App {
    pub async fn start(&self, args: &str) -> Result<()> {
        let (sender, receiver) = watch::channel(String::new());

        let server_mode = args == "server";

        spawn(run_core(server_mode, sender));
        gui::new_gui(server_mode, receiver).unwrap();
        // todo add Graceful Shutdown
        Ok(())
    }
}

async fn run_core(server_mode: bool, sender: watch::Sender<String>) -> Result<()> {
    let (watcher, shutdown_rx1) = ShutdownWatcher::new();
    let shutdown_rx2 = watcher.subscribe_shutdown();

    let discovery = Discovery::new().await?;

    let (quic_addr_channel_s, quic_addr_channel_r) = mpsc::channel(10);
    run_transfer(quic_addr_channel_r, shutdown_rx2, server_mode).await;

    return if server_mode {
        run_server(&discovery, quic_addr_channel_s, sender, shutdown_rx1).await
    } else {
        run_client(&discovery).await
    };
}

async fn run_server(
    discovery: &Discovery,
    quic_addr_channel_s: mpsc::Sender<SocketAddr>,
    sender: watch::Sender<String>,
    mut shutdown: Receiver<()>,
) -> Result<()> {
    info!("Server mode.");

    loop {
        tokio::select! {
            (addr_rs, msg) = discovery.receive_new_message() => {
                debug!("Message: {msg}");
                if let Some(addr) = addr_rs{
                    let result = quic_addr_channel_s.send(addr).await;
                    debug!("send addr: {result:?}");
                    sender.send(format!("Address: {addr:?}, Message: {msg}")).unwrap();
                }
            }
            res = shutdown.recv() => {
                debug!("Got {res:?} for shutdown the server");
                break
            }
            else => {
                warn!("Both channels closed");
                break
            }
        }
    }

    Ok(())
}

async fn run_client(discovery: &Discovery) -> Result<()> {
    info!("Client mode.");
    discovery.send_signal().await?;
    sleep(Duration::from_secs(10)).await;
    Ok(())
}

async fn run_transfer(
    mut quic_addr_channel_r: mpsc::Receiver<SocketAddr>,
    shutdown: Receiver<()>,
    mode: bool,
) {
    info!("Run Transfer");

    if mode {
        spawn(async move {
            debug!("Wait for QUIC address");
            let x = quic_addr_channel_r.recv().await;
            if let Some(addr) = x {
                debug!("GOT new address: {addr:?}");
                let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5001);
                let result = client(addr1).await;
                match result {
                    Ok(_) => {}
                    Err(e) => {
                        error!("QUIC Client error {e:?}");
                    }
                }
            }
            debug!("END quic client");
        });
    } else {
        spawn(server(shutdown));
    }

    // let server1 = server(shutdown).await;
}

mod shutdown {
    use log::debug;
    use tokio::spawn;
    use tokio::sync::broadcast::{self, Receiver, Sender};

    pub struct ShutdownWatcher(Sender<()>);

    impl ShutdownWatcher {
        pub fn new() -> (Self, Receiver<()>) {
            let (s, r) = broadcast::channel(1);
            Self::start(s.clone());
            (Self(s), r)
        }

        fn start(tx: Sender<()>) {
            spawn(async move {
                debug!("Shutdown watcher started");
                let _ = tokio::signal::ctrl_c().await;
                let r = tx.send(());
                debug!("Got ctrl+C {r:?}");
            });
        }

        pub fn subscribe_shutdown(&self) -> Receiver<()> {
            self.0.subscribe()
        }
    }
}
