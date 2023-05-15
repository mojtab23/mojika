use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use bytes::{BufMut, BytesMut};
use log::debug;
use quinn::{ClientConfig, Connection, Endpoint};
use rmp_serde::Serializer;
use serde::Serialize;
use tokio::io::AsyncReadExt;

use crate::{
    request::certificate_verifier::SkipServerVerification, request::response::Response,
    request::Request,
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

        let mut buf = BytesMut::with_capacity(1024).writer();
        request.serialize(&mut Serializer::new(&mut buf))?;

        send.write_all(buf.into_inner().as_ref()).await?;

        send.finish().await?;

        let mut buf = BytesMut::with_capacity(1024);
        let _count = recv.read_buf(&mut buf).await?;
        let response = buf.freeze().try_into()?;
        debug!("Client got response: {response:?}");
        Ok(response)
    }
}
