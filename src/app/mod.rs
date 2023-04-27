use std::{
    collections::HashMap,
    fs,
    fs::create_dir,
    net::{SocketAddr, UdpSocket},
    ops::DerefMut,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use anyhow::{bail, Error, Result};
use directories::UserDirs;
use log::{debug, error, info, warn};
use tokio::sync::OnceCell;
use tokio::{
    runtime::Runtime,
    spawn,
    sync::broadcast::Receiver,
    sync::{watch, RwLock},
    time::sleep,
};
use uuid::Uuid;

use crate::{
    app::peer::{Peer, Peers},
    app::shutdown::ShutdownWatcher,
    discovery::{Discovery, DiscoveryResult},
    gui,
    request::{
        file::CreateFile,
        file::FileTransfer,
        requester::Requester,
        responder::{server, server_addr},
        FileRequest, Request, RequestBody,
    },
};

pub mod event;
pub mod peer;

const SIGNAL_RATE: Duration = Duration::from_secs(2);
static UUID_SINGLETON: OnceCell<Uuid> = OnceCell::const_new();

pub async fn new_id() -> String {
    let id = UUID_SINGLETON
        .get_or_init(|| async { Uuid::new_v4() })
        .await;
    id.to_string()
}

#[derive(Debug)]
pub struct App {
    runtime: Runtime,
    pub peers: Arc<RwLock<Peers>>,
    pub server_port: u16,
    pub(crate) self_peer: Peer,
    requester: Arc<Requester>,
    mojika_dir: PathBuf,
    shutdown_watcher: ShutdownWatcher,
    file_transfer: Arc<FileTransfer>,
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

        let requester: Arc<Requester> = runtime
            .block_on(async { Requester::new(requester_port) })?
            .into();

        let mojika_dir = Self::create_mojika_dir()?;
        info!("Download dir: {mojika_dir:?}");

        let shutdown_watcher = runtime.block_on(async { ShutdownWatcher::new() });

        let file_transfer = FileTransfer::new(
            mojika_dir.clone(),
            requester.clone(),
            peers.clone(),
            self_peer.clone(),
        )
        .into();

        Ok(Self {
            runtime,
            peers,
            server_port,
            self_peer,
            requester,
            mojika_dir,
            shutdown_watcher,
            file_transfer,
        })
    }

    pub fn get_mojika_dir(&self) -> &PathBuf {
        &self.mojika_dir
    }

    fn create_mojika_dir() -> Result<PathBuf> {
        let user_dirs = UserDirs::new().ok_or(Error::msg("Can not find the UserDirs."))?;
        let download_dir = user_dirs
            .download_dir()
            .ok_or(Error::msg("Can not find the Download directory."))?;
        let metadata = download_dir.metadata()?;
        if !metadata.is_dir() || metadata.is_symlink() {
            bail!("Download directory is not valid. {:?}", metadata);
        }
        let mut mojika_dir = PathBuf::from(download_dir);
        mojika_dir.push("mojika");
        if !mojika_dir.exists() {
            create_dir(mojika_dir.as_path())?;
        }
        Ok(mojika_dir)
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
        let shutdown_rx1 = self.shutdown_watcher.subscribe_shutdown();
        let shutdown_rx2 = self.shutdown_watcher.subscribe_shutdown();

        let discovery = Arc::new(Discovery::new(self.self_peer.clone()).await?);

        self.clone().run_responder(shutdown_rx2).await;

        let app = self.clone();
        spawn(async move {
            app.file_transfer
                .start(app.shutdown_watcher.subscribe_shutdown())
                .await;
        });

        let d = discovery.clone();
        let app = self.clone();
        spawn(async move {
            let _ = app.run_client(&d).await;
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
            if let Some(peer_address) = write_peers.find_peer_address(&peer_id).await {
                let result = requester.request(peer_address, request).await;
                debug!("send chat result:{result:?}");
            }
        });
    }

    pub fn send_file(&self, peer_id: &str, sender_id: &str, file_path: PathBuf) {
        let Some(filename) = file_path.file_name() else {
            warn!("cant find the filename:{file_path:?}");
            return;
        };
        let Some(filename) = filename.to_str().map(|a| a.to_string()) else {
            warn!("cant get the filename as string:{filename:?}");
            return;
        };

        // read file length
        let Ok(metadata) = fs::metadata(&file_path) else {
            warn!("cant read the file metadata:{file_path:?}");
            return;
        };
        let file_length = metadata.len();

        let file = CreateFile {
            filename,
            file_length,
        };
        let peers = self.peers.clone();
        let requester = self.requester.clone();
        let peer_id = peer_id.to_string();
        let self_peer = self.self_peer.clone();
        let sender_id = sender_id.to_string();
        let file_path = file_path.to_owned();
        let file_transfer = self.file_transfer.clone();
        self.runtime.spawn(async move {
            let mut write_peers = peers.write().await;
            let create_file_request = Request::new(
                self_peer.id.to_owned(),
                self_peer.secret.to_owned(),
                RequestBody::File(FileRequest::CreateFile(file.clone())),
            );
            write_peers.add_file(&peer_id, &sender_id, file);
            if let Some(peer) = write_peers.find_by_id(&peer_id).await {
                let result = requester.request(peer.address, create_file_request).await;
                debug!("send chat result:{result:?}");
                if let Ok(r) = result {
                    if let RequestBody::File(FileRequest::FileCreated(file_id)) = r.body {
                        debug!("Got FileCreated response!");
                        file_transfer
                            .send_created_file(file_id, file_path, peer_id.clone())
                            .await
                    }
                }
            }
        });
    }

    async fn run_client(&self, discovery: &Discovery) -> Result<()> {
        info!("Sending signal.");
        let mut shutdown = self.shutdown_watcher.subscribe_shutdown();
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

    async fn run_responder(self: Arc<Self>, shutdown: Receiver<()>) {
        info!("Run Transfer");
        spawn(server(self, shutdown));
        // let server1 = server(shutdown).await;
    }

    pub async fn dispatch_request(self: Arc<Self>, request: Request) -> Request {
        let peers = self.peers.clone();
        let mut write_peers = peers.write().await;
        match request.body {
            RequestBody::Chat(chat) => {
                write_peers.add_chat(&request.peer_id, &request.peer_id, chat);
            }
            RequestBody::File(file) => {
                match file {
                    FileRequest::CreateFile(f) => {
                        let f1 = f.to_owned();
                        let Ok(file_id) = self.file_transfer.create_file(f.filename, f.file_length).await else {
                            return Request::new(
                                self.self_peer.id.clone(),
                                self.self_peer.secret.clone(),
                                RequestBody::Err("Unhandled request body!".to_string()),
                            );
                        };
                        write_peers.add_file(&request.peer_id, &request.peer_id, f1);
                        return Request::new(
                            self.self_peer.id.to_owned(),
                            self.self_peer.secret.to_owned(),
                            RequestBody::File(FileRequest::FileCreated(file_id)),
                        );
                    }
                    FileRequest::FileChunk(r) => {
                        debug!("Got file chunk request {r:?}");
                        let result = self.file_transfer.write_file_chunk(r).await;
                        // write_peers.update_file_status(&request.peer_id, &request.peer_id, f)
                        return match result {
                            Ok(_) => Request::new(
                                self.self_peer.id.to_owned(),
                                self.self_peer.secret.to_owned(),
                                RequestBody::Ok,
                            ),
                            Err(e) => {
                                warn!("error saving the file chunk:{:?}", e);
                                Request::new(
                                    self.self_peer.id.to_owned(),
                                    self.self_peer.secret.to_owned(),
                                    RequestBody::Err(e.to_string()),
                                )
                            }
                        };
                    }
                    _ => {}
                }
            }
            _ => {
                return Request::new(
                    self.self_peer.id.clone(),
                    self.self_peer.secret.clone(),
                    RequestBody::Err("Unhandled request body!".to_string()),
                );
            }
        }
        Request::new(
            self.self_peer.id.clone(),
            self.self_peer.secret.clone(),
            RequestBody::Err("Don't know how to handle your request!?".to_string()),
        )
    }
}

pub fn an_open_port() -> Result<u16> {
    let udp_socket = UdpSocket::bind("127.0.0.1:0")?;
    Ok(udp_socket.local_addr()?.port())
}

mod shutdown {
    use log::debug;
    use tokio::spawn;
    use tokio::sync::broadcast::{self, Receiver, Sender};

    #[derive(Debug)]
    pub struct ShutdownWatcher {
        sender: Sender<()>,
        _rx: Receiver<()>,
    }

    impl ShutdownWatcher {
        pub fn new() -> Self {
            let (s, r) = broadcast::channel(1);
            Self::start(s.clone());
            Self { sender: s, _rx: r }
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
            self.sender.subscribe()
        }
    }
}
