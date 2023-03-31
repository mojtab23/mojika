use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub peer_id: String,
    pub secret: String,
    pub body: RequestBody,
}

impl Request {
    pub fn new(peer_id: String, secret: String, body: RequestBody) -> Self {
        Self {
            peer_id,
            secret,
            body,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RequestBody {
    Connect,
    Chat(String),
}