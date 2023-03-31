use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use log::{debug, error, info, warn};
use tokio::runtime::Runtime;
use tokio::spawn;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::{mpsc, watch, RwLock};
use tokio::time::sleep;
use uuid::Uuid;

use crate::app::peer::{Peer, Peers};
use crate::app::shutdown::ShutdownWatcher;
use crate::discovery::{Discovery, DiscoveryResult};
use crate::gui;
use crate::request::{Request, RequestBody};
use crate::requester::Requester;
use crate::responder::{server, server_addr};

pub mod event;
pub mod peer;

const SIGNAL_RATE: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub struct App {
    runtime: Runtime,
    pub peers: Arc<RwLock<Peers>>,
    server_port: u16,
    pub(crate) self_peer: Peer,
    requester: Arc<Requester>,
}

impl App {
    pub fn new() -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        let requester_port = an_open_port().unwrap();
        let server_port = an_open_port().unwrap();
        let self_peer = Self::create_self_peer(server_port);
        info!("Start app on ports: server:{server_port}, requester:{requester_port}");
        info!("Self Peer:{self_peer:?}");
        let peers = Arc::new(RwLock::new(Peers::new(self_peer.clone())));

        let requester = runtime
            .block_on(async { Requester::new(requester_port) })?
            .into();

        Ok(Self {
            runtime,
            peers,
            server_port,
            self_peer,
            requester,
        })
    }

    fn create_self_peer(server_port: u16) -> Peer {
        let id = Uuid::new_v4().to_string();
        let secret = Uuid::new_v4().to_string();
        let name = "Buddy".to_string();
        Peer::new(id, name, secret, server_addr(server_port))
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
        let (watcher, shutdown_rx1) = ShutdownWatcher::new();
        let shutdown_rx2 = watcher.subscribe_shutdown();

        let discovery = Arc::new(Discovery::new(self.self_peer.clone()).await?);

        let (request_channel_s, mut request_channel_r) = mpsc::channel::<Request>(10);

        let peers = self.peers.clone();
        spawn(async move {
            while let Some(request) = request_channel_r.recv().await {
                if let RequestBody::Chat(chat) = request.body {
                    let mut write_peers = peers.write().await;
                    write_peers.add_chat(&request.peer_id, &request.peer_id, chat);
                }
            }
        });
        run_responder(self.server_port, request_channel_s, shutdown_rx2).await;

        let app = self.clone();

        let d = discovery.clone();
        spawn(async move {
            let _ = app.run_client(&d, watcher.subscribe_shutdown()).await;
        });
        self.run_server(&discovery.clone(), shutdown_rx1).await
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
        if dr.message.id == self.self_peer.id {
            return;
        };
        {
            let peer_id = &dr.message.id;
            let read_peers = self.peers.read().await;
            let already_peer = read_peers.find_by_id(peer_id).await;
            if already_peer.is_some() {
                return;
            }
        }
        debug!("{dr:?}");
        let a = dr.addr;
        let addr = SocketAddr::new(a.ip(), dr.message.service_port);
        let peer_id = dr.message.id;

        let new_peer = Peer::new(
            peer_id.to_owned(),
            dr.message.name.to_owned(),
            "".into(),
            addr,
        );
        {
            let mut p = self.peers.write().await;
            p.deref_mut().register(new_peer).await;
        }
        self.connect_to_peer(&peer_id);
    }

    pub fn connect_to_peer(&self, peer_id: &str) {
        let peers = self.peers.clone();
        let requester = self.requester.clone();
        let self_peer = self.self_peer.clone();
        let peer_id = peer_id.to_string();

        self.runtime.spawn(async move {
            let read_peers = peers.read().await;
            let peer_op = read_peers.find_by_id(&peer_id).await;
            match peer_op {
                None => warn!("No peer found with this ID: {peer_id}"),
                Some(peer) => {
                    debug!("Connecting peer: {peer:?}");
                    let request = Request::new(
                        self_peer.id.to_owned(),
                        self_peer.secret.to_owned(),
                        RequestBody::Connect,
                    );
                    let result = requester.request(peer.address, request).await;
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

    pub fn watch_peers(&self) -> watch::Receiver<HashMap<String, Peer>> {
        self.peers.blocking_read().watch_peers()
    }

    pub fn send_chat(&self, peer_id: &str, sender_id: &str, chat: String) {
        let peers = self.peers.clone();
        let requester = self.requester.clone();
        let peer_id = peer_id.to_string();
        let self_peer = self.self_peer.clone();
        let sender_id = sender_id.to_string();
        self.runtime.spawn(async move {
            let mut write_peers = peers.write().await;
            let request = Request::new(
                self_peer.id.to_owned(),
                self_peer.secret.to_owned(),
                RequestBody::Chat(chat.to_owned()),
            );
            write_peers.add_chat(&peer_id, &sender_id, chat);
            if let Some(peer) = write_peers.find_by_id(&peer_id).await {
                let result = requester.request(peer.address, request).await;
                debug!("send chat result:{result:?}");
            }
        });
    }

    async fn run_client(&self, discovery: &Discovery, mut shutdown: Receiver<()>) -> Result<()> {
        info!("Sending signal.");
        loop {
            tokio::select! {
                _ = sleep(SIGNAL_RATE) => {
                    discovery.send_signal().await?;
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
        info!("Signal stopped.");
        Ok(())
    }
}

async fn run_responder(port: u16, request_channel: Sender<Request>, shutdown: Receiver<()>) {
    info!("Run Transfer");
    spawn(server(port, request_channel, shutdown));
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
