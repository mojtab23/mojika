use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::net::SocketAddr;

use log::warn;
use tokio::sync::watch;
use tokio::sync::watch::{Receiver, Sender};

#[derive(Debug)]
pub struct Peers {
    self_peer: Peer,
    items: HashMap<String, Peer>,
    peers_watch_s: Sender<Vec<Peer>>,
    _peers_watch_r: Receiver<Vec<Peer>>,
}

impl Peers {
    pub fn new(self_peer: Peer) -> Self {
        let (peers_watch_s, _peers_watch_r) = watch::channel(vec![]);

        Self {
            self_peer,
            items: HashMap::with_capacity(10),
            peers_watch_s,
            _peers_watch_r,
        }
    }

    pub async fn register(&mut self, peer: Peer) -> bool {
        if peer.id == self.self_peer.id {
            return true;
        }
        let option = self.items.insert(peer.id.to_owned(), peer);
        self.items_changed();
        option.is_some()
    }

    pub async fn find_by_id(&self, id: &str) -> Option<Peer> {
        let option = self.items.get(id);
        option.cloned()
    }

    fn items_changed(&self) {
        let _ = self
            .peers_watch_s
            .send(self.items.values().cloned().collect())
            .map_err(|e| {
                warn!("Error emitting peers:{e}");
            });
    }

    pub fn watch_peers(&self) -> Receiver<Vec<Peer>> {
        self.peers_watch_s.subscribe()
    }
}

#[derive(Debug, Clone)]
pub struct Peer {
    pub id: String,
    pub name: String,
    pub address: SocketAddr,
}

impl Peer {
    pub fn new(id: String, name: String, address: SocketAddr) -> Self {
        Self { id, name, address }
    }
}

impl Display for Peer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let short_id: String = self.id.chars().take(4).collect();
        write!(f, "{} ({})", self.name, short_id)
    }
}
