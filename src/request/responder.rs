use std::net::SocketAddr;

use anyhow::Result;
use log::{debug, warn};
use quinn::{Connection, Endpoint, ServerConfig};
use tokio::io::AsyncReadExt;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::Sender;

use crate::request::Request;

fn generate_self_signed_cert() -> Result<(rustls::Certificate, rustls::PrivateKey)> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])?;
    let key = rustls::PrivateKey(cert.serialize_private_key_der());
    Ok((rustls::Certificate(cert.serialize_der()?), key))
}

pub async fn server(
    port: u16,
    request_channel: Sender<Request>,
    mut shutdown: Receiver<()>,
) -> Result<()> {
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
                receive_bidirectional_stream(connection,&request_channel).await?;
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

pub fn server_addr(port: u16) -> SocketAddr {
    format!("0.0.0.0:{port}").parse::<SocketAddr>().unwrap()
}

async fn receive_bidirectional_stream(
    connection: Connection,
    request_channel: &Sender<Request>,
) -> Result<()> {
    while let Ok((mut send, mut recv)) = connection.accept_bi().await {
        // Because it is a bidirectional stream, we can both send and receive.
        let mut msg = String::new();
        let _count = recv.read_to_string(&mut msg).await?;
        debug!("request: {msg}");
        let request = ron::from_str(&msg)?;
        request_channel.send(request).await?;
        send.write_all(b"response").await?;
        send.finish().await?;
    }

    Ok(())
}
