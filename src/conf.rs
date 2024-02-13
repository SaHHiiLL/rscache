use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    port: u16,
    child: Option<Node>,
    parent: Option<Node>,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct Node {
    addr: [u8; 4],
    port: u16,
}

impl Config {
    pub fn port(&self) -> u16 {
        self.port
    }
    pub fn child_as_addr(&self) -> SocketAddr {
        let child = self.child.clone();
        let addr = child.clone().unwrap_or_default().addr;
        SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(addr[0], addr[1], addr[2], addr[3])),
            child.unwrap_or_default().port,
        )
    }
    pub fn parent_as_addr(&self) -> SocketAddr {
        let parent = self.parent.clone();
        let addr = parent.clone().unwrap_or_default().addr;
        SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(addr[0], addr[1], addr[2], addr[3])),
            parent.unwrap_or_default().port,
        )
    }
}