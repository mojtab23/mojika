use std::fmt::{Debug, Formatter};
use std::io::SeekFrom;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Result};
use bytes::Bytes;
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use tokio::fs::OpenOptions;
use tokio::{
    fs,
    fs::File,
    io::AsyncReadExt,
    io::{AsyncSeekExt, AsyncWriteExt},
    sync::broadcast,
    sync::broadcast::{Receiver, Sender},
    sync::RwLock,
};

use crate::{
    app::{new_id, peer::Peer, peer::Peers},
    request::{requester::Requester, response::ResponseBody, FileRequest, Request, RequestBody},
};

const BUFFER_LEN: usize = 200_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateFile {
    pub filename: String,
    pub file_length: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoFile {
    pub id: String,
    pub filename: String,
    pub content_offset: u64,
    pub file_length: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileCreated {
    pub file_id: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct FileChunk {
    pub file_id: String,
    pub content_offset: u64,
    pub content: Bytes,
}

impl FileChunk {
    pub fn new(file_id: String, content_offset: u64, content: Bytes) -> Self {
        Self {
            file_id,
            content_offset,
            content,
        }
    }
}

impl Debug for FileChunk {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FileChunk ( file_id: {},  content_offset: {}, content:(len:{}))",
            self.file_id,
            self.content_offset,
            self.content.len()
        )
    }
}

#[derive(Debug)]
pub struct FileTransfer {
    mojika_dir: PathBuf,
    file_queue_sender: Sender<TransferFileCommand>,
    _file_queue_receiver: Receiver<TransferFileCommand>,
    requester: Arc<Requester>,
    peers: Arc<RwLock<Peers>>,
    self_peer: Peer,
}

impl FileTransfer {
    pub fn new(
        mojika_dir: PathBuf,
        requester: Arc<Requester>,
        peers: Arc<RwLock<Peers>>,
        self_peer: Peer,
    ) -> Self {
        let (file_queue_sender, _file_queue_receiver) = broadcast::channel(100);
        Self {
            mojika_dir,
            file_queue_sender,
            _file_queue_receiver,
            requester,
            peers,
            self_peer,
        }
    }

    pub async fn create_file(&self, filename: String, file_length: u64) -> Result<String> {
        let file_id = new_id().await;
        self.create_info_file(file_id.to_owned(), filename, file_length)
            .await?;
        self.create_download_file(file_id.to_owned()).await?;
        Ok(file_id)
    }

    async fn create_info_file(
        &self,
        file_id: String,
        filename: String,
        file_length: u64,
    ) -> Result<()> {
        let info_file_path = self.get_info_file_path(file_id.to_owned());
        if tokio::fs::try_exists(&info_file_path).await? {
            bail!("there is an existing info file:{:?}", info_file_path)
        }
        let content = InfoFile {
            id: file_id,
            filename,
            content_offset: 0,
            file_length,
        };
        let content_str = ron::to_string(&content)?;
        let mut info_file = File::create(&info_file_path).await?;
        info_file.write_all(content_str.as_bytes()).await?;
        Ok(())
    }

    fn get_info_file_path(&self, file_id: String) -> PathBuf {
        let info_filename = file_id + ".info.mojika";
        let mut info_file_path = self.mojika_dir.clone();
        info_file_path.push(info_filename);
        info_file_path
    }

    async fn read_info_file(&self, file_id: &str) -> Result<InfoFile> {
        let info_file_path = self.get_info_file_path(file_id.to_owned());
        if !tokio::fs::try_exists(&info_file_path).await? {
            bail!("info file not found:{:?}", info_file_path)
        }
        let mut info_file = File::open(&info_file_path).await?;
        let mut content = String::new();
        let _count = info_file.read_to_string(&mut content).await?;
        let info_file = ron::from_str::<InfoFile>(&content)?;
        if info_file.id != file_id {
            bail!("info file name and content doesn't match")
        }
        Ok(info_file)
    }

    async fn update_info_file(&self, info_file: InfoFile) -> Result<()> {
        debug!("update 1");

        let info_file_path = self.get_info_file_path(info_file.id.to_owned());
        if !tokio::fs::try_exists(&info_file_path).await? {
            bail!("info file not found:{:?}", info_file_path)
        }
        let content_str = ron::to_string(&info_file)?;
        debug!("update 2");

        let mut file = OpenOptions::new().write(true).open(&info_file_path).await?;
        debug!("update 3");
        file.write_all(content_str.as_bytes()).await?;
        debug!("update 4");

        file.sync_data().await?;
        debug!("info file updated: {}", content_str);
        Ok(())
    }

    async fn create_download_file(&self, file_id: String) -> Result<()> {
        let file_path = self.get_download_file_path(file_id);
        if tokio::fs::try_exists(&file_path).await? {
            bail!("there is an existing file:{:?}", file_path)
        }
        File::create(&file_path).await?;
        Ok(())
    }

    fn get_download_file_path(&self, file_id: String) -> PathBuf {
        let filename = file_id + ".mojika";
        let mut file_path = self.mojika_dir.clone();
        file_path.push(filename);
        file_path
    }

    fn get_final_file_path(&self, filename: String) -> PathBuf {
        let mut file_path = self.mojika_dir.clone();
        file_path.push(filename);
        file_path
    }

    async fn open_download_file(&self, file_id: String) -> Result<File> {
        let file_path = self.get_download_file_path(file_id);
        if !tokio::fs::try_exists(&file_path).await? {
            bail!("download file not found:{:?}", file_path)
        }
        let file = OpenOptions::new().write(true).open(&file_path).await?;

        Ok(file)
    }

    pub async fn start(&self, mut shutdown: Receiver<()>) {
        let mut file_queue_receiver = self.file_queue_sender.subscribe();
        loop {
            tokio::select! {
                Ok(tfc) = file_queue_receiver.recv() => {
                    debug!("Got tfc: {tfc:?}");
                    if let Err(e) = self.send_file_to_peer(tfc).await {
                        warn!("error transferring the file:{e:?}");
                    }
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
    }

    async fn send_file_to_peer(&self, tfc: TransferFileCommand) -> Result<()> {
        debug!("sending file to peer:{tfc:?}");
        let mut file = File::open(&tfc.file_path).await?;
        let file_len = file.metadata().await?.len();
        let mut offset = file.seek(SeekFrom::Start(tfc.content_offset)).await?;
        debug!("file seek offset:{offset}");

        let read_peers = self.peers.read().await;
        let address = read_peers
            .find_peer_address(&tfc.peer_id)
            .await
            .ok_or(anyhow::Error::msg("cant find the peer address"))?;
        drop(read_peers);
        let mut buffer = [0u8; BUFFER_LEN];
        loop {
            let count = file.read(&mut buffer).await?;

            if count == 0 {
                debug!("transfer_file complete");
                break;
            }
            let content = Bytes::copy_from_slice(&buffer[..count]);
            let file_chunk = FileChunk::new(tfc.file_id.to_owned(), offset, content);
            debug!("{file_chunk:?}");
            let request = Request::new(
                self.self_peer.id.to_owned(),
                self.self_peer.secret.to_owned(),
                RequestBody::File(FileRequest::FileChunk(file_chunk)),
            );
            let response = self.requester.request(address, request).await?;
            if let ResponseBody::Err(e) = response.body {
                warn!("Got error in response of file chunk: {}", e);
                break;
            }

            offset += count as u64;
            let percent = offset as f64 / file_len as f64 * 100.0;
            debug!("transferring percent: {percent:.1}%");
        }
        Ok(())
    }

    pub async fn send_created_file(&self, file_id: String, file_path: PathBuf, peer_id: String) {
        let tfc = TransferFileCommand {
            file_id,
            file_path,
            peer_id,
            content_offset: 0,
        };
        if let Err(e) = self.file_queue_sender.send(tfc) {
            warn!("send error in file_queue_sender {e:?}");
        }
    }

    pub async fn write_file_chunk(&self, file_chunk: FileChunk) -> Result<()> {
        let file_id = file_chunk.file_id.as_str();
        let mut info_file = self.read_info_file(file_id).await?;
        debug!("read info file: {:?}", info_file);
        // check offset
        if file_chunk.content_offset == info_file.content_offset {
            let mut file = self.open_download_file(file_id.to_string()).await?;
            let _offset = file
                .seek(SeekFrom::Start(file_chunk.content_offset))
                .await?;
            let wrote_count = file.write(file_chunk.content.as_ref()).await?;
            if wrote_count != file_chunk.content.len() {
                bail!(
                    "error in writing file chunk to file size missmatch:{:?}",
                    file_id
                )
            }
            info_file.content_offset += wrote_count as u64;

            // check if download finished
            if info_file.file_length == info_file.content_offset {
                self.finish_download_file(&info_file, file).await?;
            } else {
                self.update_info_file(info_file).await?;
            }
        } else {
            bail!(
                "file chunk and info file offset missmatch file_chunk:{}, info_file:{}",
                file_chunk.content_offset,
                info_file.content_offset
            )
        }
        Ok(())
    }

    async fn finish_download_file(&self, info_file: &InfoFile, file: File) -> Result<()> {
        debug!("finish download file:{info_file:?}");

        let actual_file_length = file.metadata().await?.len();
        if actual_file_length != info_file.file_length {
            bail!("file size missmatch, info_file:{info_file:?}, actual_file_length:{actual_file_length}")
        }

        file.sync_all().await?;
        drop(file);

        let download_path = self.get_download_file_path(info_file.id.to_owned());
        let final_path = self.get_final_file_path(info_file.filename.to_owned());
        fs::rename(&download_path, &final_path).await?;

        let info_path = self.get_info_file_path(info_file.id.to_owned());
        fs::remove_file(&info_path).await?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct TransferFileCommand {
    file_id: String,
    file_path: PathBuf,
    peer_id: String,
    content_offset: u64,
}

pub fn file_progress() {}
