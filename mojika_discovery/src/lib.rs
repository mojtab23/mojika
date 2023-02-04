use std::error::Error;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use tokio::net::UdpSocket;
use tracing::debug;

const DEFAULT_PORT: u16 = 10020;

pub struct Discovery {
    socket: Arc<UdpSocket>,
}

impl Discovery {
    pub async fn new() -> Result<Self, Box<dyn Error>> {
        let socket = Self::create_socket()?;
        let socket = Arc::new(socket);
        let discovery = Self { socket };
        Ok(discovery)
    }

    fn create_socket() -> Result<UdpSocket, Box<dyn Error>> {
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
                let msg = String::from_utf8_lossy(&buf[..len]);
                debug!("message form address: {addr:?}");
                msg.to_string()
            }
            Err(_) => "Error!".to_string(),
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn it_works() {
//         let result = add(2, 2);
//         assert_eq!(result, 4);
//     }
// }
