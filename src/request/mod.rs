use bytes::{Buf, Bytes};
use serde::{Deserialize, Serialize};

use crate::request::file::{CreateFile, FileChunk};

mod certificate_verifier;
pub mod file;
pub mod protocol;
pub mod requester;
pub mod responder;
pub mod response;

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

impl TryFrom<Bytes> for Request {
    type Error = anyhow::Error;

    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        let mut deserializer = rmp_serde::Deserializer::new(value.reader());
        Ok(Deserialize::deserialize(&mut deserializer)?)
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum RequestBody {
    Connect,
    Chat(String),
    File(FileRequest),
    Ok,
    Err(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FileRequest {
    CreateFile(CreateFile),
    FileCreated(String),
    FileChunk(FileChunk),
}
