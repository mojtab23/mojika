use anyhow::Result;
use bytes::{Buf, Bytes};
use serde::{Deserialize, Serialize};

use crate::request::file::{CreateFile, FileChunk};

pub mod file;
pub mod requester;
pub mod responder;

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

pub fn deserialize(buf: Bytes) -> Result<Request> {
    let mut deserializer = rmp_serde::Deserializer::new(buf.reader());
    Ok(Deserialize::deserialize(&mut deserializer)?)
}
