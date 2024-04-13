use anyhow::{bail, Result};
use bytes::Bytes;
use log::{debug, error, warn};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader};

pub const PROTOCOL_HEADER_MAX_LEN: usize = 2048;

#[derive(Debug)]
pub struct MojikaProtocol {
    pub header: MojikaProtocolHeader,
    pub content: Bytes,
}

#[derive(Debug)]
pub struct MojikaProtocolHeader {
    pub type_name: String,
    pub len: usize,
}

impl MojikaProtocol {
    pub fn new(header: &str, content: Bytes) -> Result<Self> {
        let header = parse_header(header)?;
        Ok(Self { header, content })
    }
    pub async fn from_read(read: impl AsyncRead + Unpin) -> Result<Self> {
        let mut buf_reader = BufReader::with_capacity(PROTOCOL_HEADER_MAX_LEN, read);
        debug!("from_read");
        let mut header = vec![];
        let header_len = buf_reader.read_until(b'\n', &mut header).await;
        match header_len {
            Ok(header_len) => debug!("header_len:{:?}", header_len),
            Err(e) => {
                error!("errrrr:{:?}", e)
            }
        };
        debug!(" buf_reader.read_until");
        let header = String::from_utf8(header)?;
        debug!("header:{header}");

        let header = parse_header(&header)?;
        debug!("header:{:?}", &header);

        let mut content = Vec::with_capacity(header.len);

        let _content_len = buf_reader.read_to_end(&mut content).await?;
        debug!("buf_reader.read_to_end:{:?}", &header);
        Ok(Self {
            header,
            content: Bytes::from(content),
        })
    }
}

impl MojikaProtocolHeader {
    pub fn serialize(&self) -> String {
        format!("type_name={},len={}\n", &self.type_name, &self.len)
    }
}

fn parse_header(header: &str) -> Result<MojikaProtocolHeader> {
    let splits = header.trim().split(',');
    let mut type_name = None;
    let mut len = None;
    for part in splits {
        let mut key_value = part.split('=');
        let key = match key_value.next() {
            None => bail!("key not found in header"),
            Some(k) => k,
        };
        debug!("header key:{key}");
        let value = match key_value.next() {
            None => bail!("value not found in header"),
            Some(v) => v,
        };
        debug!("header value:{value}");

        match key {
            "type_name" => type_name = Some(value.to_string()),
            "len" => len = Some(value.parse::<usize>()?),
            _ => warn!("invalid header key:{key}"),
        }
    }

    if type_name.is_none() {
        bail!("header 'type_name' is missing")
    };
    if len.is_none() {
        bail!("header 'len' is missing")
    };
    Ok(MojikaProtocolHeader {
        type_name: type_name.unwrap(),
        len: len.unwrap(),
    })
}

pub trait MojikaContent {
    fn type_name() -> String;
    fn len() -> usize;
}

#[cfg(test)]
mod tests {
    use crate::request::protocol::{MojikaProtocol, MojikaProtocolHeader};

    #[tokio::test]
    async fn protocol_header_from_read() {
        let content = "test content";
        let header = MojikaProtocolHeader {
            type_name: "Request".to_string(),
            len: content.len(),
        };
        let mut header_str = header.serialize();
        header_str.push_str(content);
        let result = MojikaProtocol::from_read(header_str.as_bytes()).await;
        assert!(result.is_ok());
        let header_result = result.unwrap().header;
        assert_eq!(header_result.type_name, "Request");
        assert_eq!(header_result.len, content.len());
    }
}
