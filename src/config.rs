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

impl Node {
    pub fn into(self) -> SocketAddr {
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

    pub fn port(&self) -> u16 {
        self.port.unwrap_or_else(|| {
            std::env::var("RSCACHE_PORT")
                .map(|p| {
                    p.parse()
                        .expect("Unable to parse `RSCACHE_PORT` -> not a valid `u16`")
                })
                .expect("Could not find fallback port set `RSCACHE_PORT`")
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
