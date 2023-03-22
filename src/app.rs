use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::Duration;

use anyhow::Result;
use log::{debug, error, info, warn};
use tokio::spawn;
use tokio::sync::broadcast::Receiver;
use tokio::sync::{mpsc, watch};
use tokio::time::sleep;

use crate::app::shutdown::ShutdownWatcher;
use crate::connect::{client, server};
use crate::discovery::{Discovery, DiscoveryResult};
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

    let port = an_open_port()?;
    let discovery = Discovery::new(port).await?;

    let (quic_addr_channel_s, quic_addr_channel_r) = mpsc::channel(10);
    run_transfer(port, quic_addr_channel_r, shutdown_rx2, server_mode).await;

    return if server_mode {
        run_server(&discovery, quic_addr_channel_s, sender, shutdown_rx1).await
    } else {
        run_client(&discovery).await
    };
}

async fn run_server(
    discovery: &Discovery,
    quic_addr_channel_s: mpsc::Sender<DiscoveryResult>,
    sender: watch::Sender<String>,
    mut shutdown: Receiver<()>,
) -> Result<()> {
    info!("Server mode.");

    loop {
        tokio::select! {
            Ok(dis) = discovery.receive_new_message() => {
                debug!("{dis:?}");
                let a = dis.addr;
                let b = &dis.message;
                sender.send(format!("Address: {a:?}, Message: {b}")).unwrap();
                let result = quic_addr_channel_s.send(dis).await;
                debug!("send addr: {result:?}");
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
    port: u16,
    mut quic_addr_channel_r: mpsc::Receiver<DiscoveryResult>,
    shutdown: Receiver<()>,
    mode: bool,
) {
    info!("Run Transfer");

    if mode {
        spawn(async move {
            debug!("Wait for QUIC address");
            if let Some(dis) = quic_addr_channel_r.recv().await {
                debug!("GOT new peer: {dis:?}");
                let addr1 = SocketAddr::new(
                    IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                    dis.message.service_port,
                );
                let result = client(addr1, port).await;
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
        spawn(server(port, shutdown));
    }

    // let server1 = server(shutdown).await;
}

pub fn an_open_port() -> Result<u16> {
    let udp_socket = UdpSocket::bind("127.0.0.1:0")?;
    Ok(udp_socket.local_addr()?.port())
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
