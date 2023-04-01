use std::sync::Arc;

use bytes::Bytes;

use crate::app::App;

pub struct CreateFile {
    filename: String,
    file_length: u64,
}

pub struct FileCreated {
    file_id: String,
}

pub struct FileChunk {
    file_id: String,
    content_offset: u64,
    content: Bytes,
}

pub struct FileTransfer {
    app: Arc<App>,
    send_file_queue: Vec<String>,
}

impl FileTransfer {
    pub fn new(app: Arc<App>) -> Self {
        Self {
            app: app.clone(),
            send_file_queue: vec![],
        }
    }

    pub async fn send_file(&mut self) {
        //     TODO continue from here!
    }
}
