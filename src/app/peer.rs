use std::{
    collections::hash_map::Entry::Vacant,
    collections::HashMap,
    fmt::{Display, Formatter},
    net::SocketAddr,
};

use log::warn;
use tokio::sync::{
    watch,
    watch::{Receiver, Sender},
};
use uuid::Uuid;

use crate::{
    chat::{Chat, Message},
    request::file::CreateFile,
};

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

    pub async fn find_peer_address(&self, peer_id: &str) -> Option<SocketAddr> {
        self.find_by_id(peer_id).await.map(|p| p.address)
    }

    pub fn add_chat(&mut self, peer_id: &str, sender_id: &str, chat: String) {
        let peer_op = self.items.get_mut(peer_id);
        match peer_op {
            None => {}
            Some(peer) => {
                peer.chat.messages.push(Message::new_text(sender_id, chat));
                self.items_changed();
            }
        }
    }

    pub fn add_file(&mut self, peer_id: &str, sender_id: &str, file: CreateFile) {
        let peer_op = self.items.get_mut(peer_id);

        if let Some(peer) = peer_op {
            peer.chat.messages.push(Message::new_file(
                sender_id,
                Uuid::new_v4().to_string(),
                file.filename,
                "Just created.".to_string(),
            ));
            self.items_changed();
        }
    }
    // pub fn update_file_status(&mut self, peer_id: &str, sender_id: &str, file_chunk: FileChunk) {
    //     let peer_op = self.items.get_mut(peer_id);
    //     if let Some(peer) = peer_op {
    //         let iter = peer.chat.messages.iter();
    //         let option = iter.find(|&a| a.id == file_chunk.file_id);
    //
    //         peer.chat.messages.push(Message::new_file(sender_id, file));
    //         self.items_changed();
    //     }
    // }

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
    pub secret: String,
    pub address: SocketAddr,
    pub chat: Chat,
}

impl Peer {
    pub fn new(id: String, name: String, secret: String, address: SocketAddr) -> Self {
        Self {
            id,
            name,
            address,
            secret,
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
