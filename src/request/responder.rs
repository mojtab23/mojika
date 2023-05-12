use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use bytes::{BufMut, BytesMut};
use log::{debug, warn};
use quinn::{Connection, Endpoint, ServerConfig};
use rmp_serde::Serializer;
use serde::Serialize;
use tokio::io::AsyncReadExt;
use tokio::sync::broadcast::Receiver;

use crate::app::App;

fn generate_self_signed_cert() -> Result<(rustls::Certificate, rustls::PrivateKey)> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])?;
    let key = rustls::PrivateKey(cert.serialize_private_key_der());
    Ok((rustls::Certificate(cert.serialize_der()?), key))
}

pub async fn server(
    app: Arc<App>,
    // request_channel: Sender<Request>,
    mut shutdown: Receiver<()>,
) -> Result<()> {
    // Bind this endpoint to a UDP socket on the given server address.
    let (cer, pvk) = generate_self_signed_cert()?;
    let config = ServerConfig::with_single_cert(vec![cer], pvk)?;
    let endpoint = Endpoint::server(config, server_addr(app.server_port))?;

    debug!("Start QUIC server on:{:?}", endpoint.local_addr());

    loop {
        tokio::select! {
            Some(conn) = endpoint.accept() => {
                let connection = conn.await?;
                let client_addr = connection.remote_address();
                debug!("Got new QUIC connection {client_addr:?}");
                // Save connection somewhere, start transferring, receiving data, see DataTransfer tutorial.
                receive_bidirectional_stream(connection, app.clone()).await?;
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
    // request_channel: &Sender<Request>,
    app: Arc<App>,
) -> Result<()> {
    while let Ok((mut send, mut recv)) = connection.accept_bi().await {
        let app = app.clone();
        // Because it is a bidirectional stream, we can both send and receive.
        let mut buf = BytesMut::with_capacity(10_000);
        let _count = recv.read_buf(&mut buf).await?;
        let request = buf.freeze().try_into()?;
        debug!("request: {request:?}");
        let response = app.dispatch_request(request).await;
        // request_channel.send(request).await?;
        let mut message_bytes = BytesMut::with_capacity(1024).writer();
        response.serialize(&mut Serializer::new(&mut message_bytes))?;
        send.write_all(&message_bytes.into_inner().freeze()).await?;
        // send.write_chunk(message_bytes.into_inner().freeze());
        // send.write_all(b"response").await?;
        send.finish().await?;
    }
    Ok(())
}
