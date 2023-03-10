use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;

use anyhow::Result;
use log::{debug, info};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use tokio::net::UdpSocket;
use uuid::Uuid;

const DEFAULT_PORT: u16 = 10020;

pub struct Discovery {
    socket: Arc<UdpSocket>,
}

impl Discovery {
    pub async fn new() -> Result<Self> {
        let socket = Self::create_socket()?;
        let socket = Arc::new(socket);
        let discovery = Self { socket };
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

    pub async fn receive_new_message(&self) -> String {
        let socket: Arc<UdpSocket> = self.socket.clone();
        let mut buf = vec![0u8; 1024];
        let result = socket.recv_from(&mut buf).await;
        match result {
            Ok((len, addr)) => {
                let msg = Uuid::from_slice(&buf[..len])
                    .map(|u| u.to_string())
                    .unwrap_or("Invalid UUID".to_string());
                // let msg = String::from_utf8_lossy(&buf[..len]);
                debug!("message form address: {addr:?}");
                msg
            }
            Err(_) => "Error!".to_string(),
        }
    }

    pub async fn send_signal(&self) -> Result<()> {
        let socket: Arc<UdpSocket> = self.socket.clone();
        let client_id = Uuid::new_v4();
        let msg = client_id.as_bytes();
        let addr = SocketAddrV4::new(Ipv4Addr::new(224, 0, 1, 1), DEFAULT_PORT);
        let len = socket.send_to(msg, &addr).await?;
        info!("Client Sent {len} bytes.");
        Ok(())
    }
}
