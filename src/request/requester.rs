use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use bytes::{BufMut, BytesMut};
use log::{debug, error};
use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream};
use rmp_serde::Serializer;
use serde::Serialize;

use crate::{
    request::certificate_verifier::SkipServerVerification,
    request::protocol::{MojikaProtocol, MojikaProtocolHeader},
    request::Request,
    request::response::Response
};

#[derive(Debug)]
pub struct Requester {
    endpoint: Endpoint,
}

impl Requester {
    pub fn new(port: u16) -> Result<Self> {
        // Bind this endpoint to a UDP socket on the given client address.
        let mut endpoint = Endpoint::client(Self::client_addr(port))?;
        endpoint.set_default_client_config(Self::configure_client());
        Ok(Self { endpoint })
    }

    fn client_addr(port: u16) -> SocketAddr {
        format!("0.0.0.0:{port}").parse::<SocketAddr>().unwrap()
    }
    fn configure_client() -> ClientConfig {
        let crypto = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(SkipServerVerification::new())
            .with_no_client_auth();

        ClientConfig::new(Arc::new(crypto))
    }

    pub async fn request(&self, remote_addr: SocketAddr, request: Request) -> Result<Response> {
        debug!("Connecting server:{remote_addr:?}");
        // Connect to the server passing in the server name which is supposed to be in the server certificate.
        let connecting = self.endpoint.connect(remote_addr, "localhost")?;
        let connection = connecting.await?;
        // Start transferring, receiving data, see data transfer page.
        let response = Self::open_bidirectional_stream(connection, request).await?;
        Ok(response)
    }

    async fn open_bidirectional_stream(
        connection: Connection,
        request: Request,
    ) -> Result<Response> {
        let (mut send, mut recv) = connection.open_bi().await?;
        send_request(&mut send, &request).await?;
        let response = receive_response(&mut recv).await?;
        debug!("Client got response: {response:?}");
        Ok(response)
    }
}

async fn send_request(send: &mut SendStream, request: &Request) -> Result<()> {
    let mut bytes = BytesMut::with_capacity(1024).writer();
    request.serialize(&mut Serializer::new(&mut bytes))?;
    let bytes = bytes.into_inner();

    let header = MojikaProtocolHeader {
        type_name: "Request".to_string(),
        len: bytes.len(),
    };
    let mut all = BytesMut::from(header.serialize().as_bytes());
    all.put(bytes);
    send.write_all(all.as_ref()).await?;
    send.finish().await?;
    Ok(())
}

async fn receive_response(recv: &mut RecvStream) -> Result<Response> {
    let protocol = MojikaProtocol::from_read(recv).await?;
    if protocol.header.type_name != "Response" {
        error!("Invalid type_name expect 'Response'");
    };
    let response = protocol.content.try_into()?;
    Ok(response)
}
