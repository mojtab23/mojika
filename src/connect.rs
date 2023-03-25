use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use log::{debug, warn};
use quinn::{ClientConfig, Connection, Endpoint, ServerConfig};
use tokio::sync::broadcast::Receiver;

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

fn configure_client() -> ClientConfig {
    let crypto = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_custom_certificate_verifier(SkipServerVerification::new())
        .with_no_client_auth();

    ClientConfig::new(Arc::new(crypto))
}

fn generate_self_signed_cert() -> Result<(rustls::Certificate, rustls::PrivateKey)> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])?;
    let key = rustls::PrivateKey(cert.serialize_private_key_der());
    Ok((rustls::Certificate(cert.serialize_der()?), key))
}

pub async fn server(port: u16, mut shutdown: Receiver<()>) -> Result<()> {
    // Bind this endpoint to a UDP socket on the given server address.
    let (cer, pvk) = generate_self_signed_cert()?;
    let config = ServerConfig::with_single_cert(vec![cer], pvk)?;
    let endpoint = Endpoint::server(config, server_addr(port))?;

    debug!("Start QUIC server on:{:?}", endpoint.local_addr());

    loop {
        tokio::select! {
            Some(conn) = endpoint.accept() => {
                let connection = conn.await?;
                let client_addr = connection.remote_address();
                debug!("Got new QUIC connection {client_addr:?}");
                // Save connection somewhere, start transferring, receiving data, see DataTransfer tutorial.
                receive_bidirectional_stream(connection).await?;
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
    Ok(())
}

pub async fn client(remote_addr: SocketAddr, port: u16) -> Result<()> {
    // Bind this endpoint to a UDP socket on the given client address.
    let mut endpoint = Endpoint::client(client_addr(port))?;
    endpoint.set_default_client_config(configure_client());

    debug!("Connecting server:{remote_addr:?}");
    // Connect to the server passing in the server name which is supposed to be in the server certificate.
    let connection = endpoint.connect(remote_addr, "localhost")?.await?;
    // Start transferring, receiving data, see data transfer page.
    open_bidirectional_stream(connection).await?;

    Ok(())
}

fn client_addr(port: u16) -> SocketAddr {
    format!("0.0.0.0:{port}").parse::<SocketAddr>().unwrap()
}

pub fn server_addr(port: u16) -> SocketAddr {
    format!("0.0.0.0:{port}").parse::<SocketAddr>().unwrap()
}

async fn open_bidirectional_stream(connection: Connection) -> Result<()> {
    let (mut send, recv) = connection.open_bi().await?;

    send.write_all(b"test").await?;
    send.finish().await?;

    let received = recv.read_to_end(10).await?;
    let receive = String::from_utf8_lossy(&received);
    debug!("Client got: {receive}");
    Ok(())
}

async fn receive_bidirectional_stream(connection: Connection) -> Result<()> {
    while let Ok((mut send, recv)) = connection.accept_bi().await {
        // Because it is a bidirectional stream, we can both send and receive.
        let vec = recv.read_to_end(50).await?;
        let msg = String::from_utf8_lossy(&vec);
        debug!("request: {msg}");

        send.write_all(b"response").await?;
        send.finish().await?;
    }

    Ok(())
}
