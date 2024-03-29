use std::fmt::{Display, Formatter};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    port: Option<u16>,
    parent: Option<Node>,
    cleanup_time: Option<u16>,
    max_nodes: Option<u16>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Node {
    addr: [u8; 4],
    port: u16,
}

impl Display for Node {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}",
            self.addr
                .to_vec()
                .iter()
                .map(|z| z.to_string())
                .collect::<Vec<_>>()
                .join(","),
            self.port
        )
    }
}

impl Node {
    pub fn to_socketaddr(&self) -> SocketAddr {
        SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(
                self.addr[0],
                self.addr[1],
                self.addr[2],
                self.addr[3],
            )),
            self.port,
        )
    }

    pub fn addr(&self) -> [u8; 4] {
        self.addr
    }
}

impl Config {
    pub fn max_nodes(&self) -> u16 {
        self.max_nodes.unwrap_or(3)
    }

    /// Looks for `RSCACHE_PORT` in the environment if the port is not set in the config
    /// returns 0 if no port is set
    pub fn port(&self) -> u16 {
        self.port.unwrap_or_else(|| {
            std::env::var("RSCACHE_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(0) // should just indicate to the OS to pick a port
        })
    }

    pub fn parent(&self) -> Option<Node> {
        self.parent.clone()
    }

    pub fn cleanup_time_as_duration(&self) -> Duration {
        let ct = self.cleanup_time.unwrap_or(60);
        Duration::from_secs(ct.into())
    }
}
