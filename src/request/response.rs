use bytes::{Buf, Bytes};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub peer_id: String,
    pub secret: String,
    pub body: ResponseBody,
}

impl Response {
    pub fn new(peer_id: String, secret: String, body: ResponseBody) -> Self {
        Self {
            peer_id,
            secret,
            body,
        }
    }

    pub fn create_ok_response(peer_id: String, secret: String) -> Self {
        Self::new(peer_id, secret, ResponseBody::Ok)
    }
}

impl TryFrom<Bytes> for Response {
    type Error = anyhow::Error;
    fn try_from(value: Bytes) -> std::result::Result<Self, Self::Error> {
        let mut deserializer = rmp_serde::Deserializer::new(value.reader());
        Ok(Deserialize::deserialize(&mut deserializer)?)
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum ResponseBody {
    File(FileResponse),
    Ok,
    Err(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FileResponse {
    FileCreated(String),
}
