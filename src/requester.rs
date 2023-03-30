use std::net::SocketAddr;
use std::sync::Arc;

use crate::request::Request;
use anyhow::Result;
use log::debug;
use quinn::{ClientConfig, Connection, Endpoint};

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

    pub async fn request(&self, remote_addr: SocketAddr, request: Request) -> Result<()> {
        debug!("Connecting server:{remote_addr:?}");
        // Connect to the server passing in the server name which is supposed to be in the server certificate.
        let connecting = self.endpoint.connect(remote_addr, "localhost")?;
        let connection = connecting.await?;
        // Start transferring, receiving data, see data transfer page.
        Self::open_bidirectional_stream(connection, request).await?;
        Ok(())
    }

    async fn open_bidirectional_stream(connection: Connection, request: Request) -> Result<()> {
        let (mut send, recv) = connection.open_bi().await?;

        let data = ron::to_string(&request)?;
        send.write_all(data.as_bytes()).await?;

        send.finish().await?;

        let received = recv.read_to_end(1024).await?;

        let receive = String::from_utf8_lossy(&received);
        debug!("Client got: {receive}");
        Ok(())
    }
}

// Implementation of `ServerCertVerifier` that verifies everything as trustworthy.
struct SkipServerVerification;

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl rustls::client::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}
