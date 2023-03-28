use std::collections::hash_map::Entry::Vacant;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::net::SocketAddr;

use log::warn;
use tokio::sync::watch;
use tokio::sync::watch::{Receiver, Sender};

use crate::chat::{Chat, Message};

#[derive(Debug)]
pub struct Peers {
    self_peer: Peer,
    items: HashMap<String, Peer>,
    peers_watch_s: Sender<HashMap<String, Peer>>,
    _peers_watch_r: Receiver<HashMap<String, Peer>>,
}

impl Peers {
    pub fn new(self_peer: Peer) -> Self {
        let (peers_watch_s, _peers_watch_r) = watch::channel(HashMap::default());

        Self {
            self_peer,
            items: HashMap::with_capacity(10),
            peers_watch_s,
            _peers_watch_r,
        }
    }

    pub async fn register(&mut self, peer: Peer) {
        if peer.id == self.self_peer.id {
            return;
        }
        let peer_id = peer.id.to_owned();
        if let Vacant(e) = self.items.entry(peer_id) {
            e.insert(peer);
            self.items_changed();
        }
    }

    pub async fn find_by_id(&self, id: &str) -> Option<Peer> {
        let option = self.items.get(id);
        option.cloned()
    }

    pub fn add_chat(&mut self, peer_id: &str, sender_id: &str, chat: String) {
        let peer_op = self.items.get_mut(peer_id);
        match peer_op {
            None => {}
            Some(peer) => {
                peer.chat.messages.push(Message::new(sender_id, chat));
                self.items_changed();
            }
        }
    }

    fn items_changed(&self) {
        let _ = self.peers_watch_s.send(self.items.clone()).map_err(|e| {
            warn!("Error emitting peers:{e}");
        });
    }

    pub fn watch_peers(&self) -> Receiver<HashMap<String, Peer>> {
        self.peers_watch_s.subscribe()
    }
}

#[derive(Debug, Clone)]
pub struct Peer {
    pub id: String,
    pub name: String,
    pub address: SocketAddr,
    pub chat: Chat,
}

impl Peer {
    pub fn new(id: String, name: String, address: SocketAddr) -> Self {
        Self {
            id,
            name,
            address,
            chat: Chat::new(),
        }
    }
}

impl Display for Peer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let short_id: String = self.id.chars().take(4).collect();
        write!(f, "{} ({})", self.name, short_id)
    }
}
