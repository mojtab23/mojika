use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use bytes::{BufMut, BytesMut};
use log::{debug, warn};
use quinn::{Connection, Endpoint, RecvStream, SendStream, ServerConfig};
use rmp_serde::Serializer;
use serde::Serialize;
use tokio::sync::broadcast::Receiver;

use crate::app::App;
use crate::request::protocol::{MojikaProtocol, MojikaProtocolHeader};
use crate::request::response::Response;
use crate::request::Request;

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
        let request = receive_request(&mut recv).await?;
        let response = app.dispatch_request(request).await;
        send_response(&mut send, &response).await?;
    }
    Ok(())
}

async fn receive_request(recv: &mut RecvStream) -> Result<Request> {
    let protocol = MojikaProtocol::from_read(recv).await?;
    let request = protocol.content.try_into()?;
    Ok(request)
}

async fn send_response(send: &mut SendStream, response: &Response) -> Result<()> {
    let mut bytes = BytesMut::with_capacity(1024).writer();
    response.serialize(&mut Serializer::new(&mut bytes))?;
    let bytes = bytes.into_inner();
    let len = bytes.len();
    let header = MojikaProtocolHeader {
        type_name: "Response".to_string(),
        len,
    };
    send.write_all(header.serialize().as_bytes()).await?;
    send.write_all(&bytes).await?;
    send.finish().await?;
    Ok(())
}
