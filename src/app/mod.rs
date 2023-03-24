use std::net::{SocketAddr, UdpSocket};
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use log::{debug, error, info, warn};
use tokio::runtime::Runtime;
use tokio::spawn;
use tokio::sync::broadcast::Receiver;
use tokio::sync::{watch, RwLock};
use tokio::time::sleep;

use crate::app::peer::{Peer, Peers};
use crate::app::shutdown::ShutdownWatcher;
use crate::connect::{client, server};
use crate::discovery::{Discovery, DiscoveryResult};
use crate::gui;

pub mod event;
pub mod peer;

#[derive(Debug)]
pub struct App {
    runtime: Runtime,
    pub server_mode: bool,
    pub peers: Arc<RwLock<Peers>>,
    client_port: u16,
    server_port: u16,
}

impl App {
    pub fn new(args: &str) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        let client_port = an_open_port().unwrap();
        let server_port = an_open_port().unwrap();
        info!("Start app on ports: server:{server_port}, client:{client_port}");
        let server_mode = args == "server";
        let peers = Arc::new(RwLock::new(Peers::new()));
        Self {
            runtime,
            server_mode,
            peers,
            client_port,
            server_port,
        }
    }

    pub fn start(self: Arc<Self>) -> Result<()> {
        self.runtime.spawn(self.clone().run_core());

        // spawn(self.clone().run_core(sender));
        gui::new_gui(self).unwrap();
        // todo add Graceful Shutdown
        Ok(())
    }

    // todo fix
    // pub fn stop(self) {
    //     self.runtime.shutdown_timeout(Duration::from_secs(10));
    // }

    async fn run_core(self: Arc<Self>) -> Result<()> {
        let server_mode = self.server_mode;
        let (watcher, shutdown_rx1) = ShutdownWatcher::new();
        let shutdown_rx2 = watcher.subscribe_shutdown();

        let discovery = Discovery::new(self.server_port).await?;

        run_transfer(self.server_port, shutdown_rx2, server_mode).await;

        return if server_mode {
            self.run_server(&discovery, shutdown_rx1).await
        } else {
            run_client(&discovery).await
        };
    }

    async fn run_server(&self, discovery: &Discovery, mut shutdown: Receiver<()>) -> Result<()> {
        info!("Server mode.");

        loop {
            tokio::select! {
                Ok(dr) = discovery.receive_new_message() => {
                    self.handle_new_message(dr).await;
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

    async fn handle_new_message(&self, dr: DiscoveryResult) {
        debug!("{dr:?}");
        let a = dr.addr;
        let _b = &dr.message;
        let addr = SocketAddr::new(a.ip(), dr.message.service_port);
        let peer_id = dr.message.id;
        let new_peer = Peer::new(peer_id.to_owned(), dr.message.name.to_owned(), addr);
        {
            let mut p = self.peers.write().await;
            p.deref_mut().register(new_peer).await;
        }
        self.connect_to_peer(&peer_id);
    }

    pub fn connect_to_peer(&self, peer_id: &str) {
        let peers = self.peers.clone();
        let client_port = self.client_port;
        let peer_id = peer_id.to_string();

        self.runtime.spawn(async move {
            let read_peers = peers.read().await;
            let peer_op = read_peers.find_by_id(&peer_id).await;
            match peer_op {
                None => warn!("No peer found with this ID: {peer_id}"),
                Some(peer) => {
                    debug!("Connecting peer: {peer:?}");
                    let result = client(peer.address, client_port).await;
                    match result {
                        Ok(_) => {}
                        Err(e) => {
                            error!("QUIC Client error {e:?}");
                        }
                    }
                }
            }
        });
    }

    pub fn watch_peers(&self) -> watch::Receiver<Vec<Peer>> {
        self.peers.blocking_read().watch_peers()
    }
}

async fn run_client(discovery: &Discovery) -> Result<()> {
    info!("Client mode.");
    discovery.send_signal().await?;
    sleep(Duration::from_secs(10)).await;
    Ok(())
}

async fn run_transfer(port: u16, shutdown: Receiver<()>, mode: bool) {
    info!("Run Transfer");

    if !mode {
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
