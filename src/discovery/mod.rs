use std::fmt::{Display, Formatter};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;

use anyhow::{bail, Result};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use tokio::net::UdpSocket;
use uuid::Uuid;

const DEFAULT_PORT: u16 = 10020;

pub struct Discovery {
    socket: Arc<UdpSocket>,
    message: DiscoveryMessage,
}

impl Discovery {
    pub async fn new(service_port: u16) -> Result<Self> {
        let socket = Self::create_socket()?;
        let socket = Arc::new(socket);
        let message = DiscoveryMessage::new(service_port);
        let discovery = Self { socket, message };
        Ok(discovery)
    }

    fn create_socket() -> Result<UdpSocket> {
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        socket.set_reuse_address(true)?;
        socket.set_nonblocking(true)?;
        let multiaddr = Ipv4Addr::new(224, 0, 1, 1);
        socket.set_multicast_loop_v4(true)?;
        let addr = SocketAddr::new("0.0.0.0".parse()?, DEFAULT_PORT);
        socket.bind(&SockAddr::from(addr))?;
        let interface = socket2::InterfaceIndexOrAddress::Index(0);
        socket.join_multicast_v4_n(&multiaddr, &interface)?;
        let std_socket: std::net::UdpSocket = socket.into();
        Ok(UdpSocket::from_std(std_socket)?)
    }

    pub async fn receive_new_message(&self) -> Result<DiscoveryResult> {
        let socket: Arc<UdpSocket> = self.socket.clone();
        let mut buf = vec![0u8; 1024];
        let result = socket.recv_from(&mut buf).await;
        match result {
            Ok((len, addr)) => {
                let msg = String::from_utf8_lossy(&buf[..len]);
                let discovery_msg = ron::from_str::<DiscoveryMessage>(&msg)?;

                debug!("message form address: {addr:?}");
                Ok(DiscoveryResult::new(discovery_msg, addr))
            }
            Err(e) => {
                bail!("Can't read the message: {}", e)
            }
        }
    }

    pub async fn send_signal(&self) -> Result<()> {
        let socket: Arc<UdpSocket> = self.socket.clone();
        let message = ron::to_string(&self.message)?;
        let msg = message.as_bytes();
        info!("{message:?}");
        let addr = SocketAddrV4::new(Ipv4Addr::new(224, 0, 1, 1), DEFAULT_PORT);
        let len = socket.send_to(msg, &addr).await?;
        debug!("Client Sent {len} bytes.");
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiscoveryMessage {
    pub id: String,
    pub name: String,
    pub service_port: u16,
}

impl DiscoveryMessage {
    pub fn new(service_port: u16) -> Self {
        let id = Uuid::new_v4().to_string();
        Self {
            id,
            name: "Buddy".to_string(),
            service_port,
        }
    }
}

impl Display for DiscoveryMessage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let id = if self.id.len() > 4 {
            &self.id[0..4]
        } else {
            &self.id
        };
        write!(f, "{} ({}) on port {}", self.name, id, self.service_port)
    }
}

#[derive(Debug)]
pub struct DiscoveryResult {
    pub message: DiscoveryMessage,
    pub addr: SocketAddr,
}

impl DiscoveryResult {
    fn new(message: DiscoveryMessage, addr: SocketAddr) -> Self {
        Self { message, addr }
    }
}
