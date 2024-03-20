#![deny(unused_must_use)]
#![allow(clippy::let_underscore_future)]
use std::{net::SocketAddr, process::exit, str::FromStr, sync::Arc};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, info_span, trace_span, Level};

use crate::client::Client;

mod client;
mod config;
mod database;
mod message;
mod server;

pub async fn connect_to_parent(
    cfg: &crate::config::Config,
) -> Result<tokio::net::TcpStream, Box<dyn std::error::Error>> {
    let parent = cfg.parent().ok_or("No parent set")?;

    let mut connection =
        tokio::net::TcpStream::connect::<SocketAddr>(parent.to_socketaddr()).await?;

    // By making the connection, we are currently registered as a normal "client" to the parent and
    // not as Node. The parent is expecting a "JOIN" message from us to register us as a node.
    connection.write_all(b"JOIN").await?;
    // Now we must wait for the parent to respond
    let mut buf = [0; 1024];

    let timeout = tokio::time::Duration::from_secs(10);
    if let Ok(n) = tokio::time::timeout(timeout, connection.read(&mut buf)).await? {
        let response = std::str::from_utf8(&buf[..n])?;
        match response {
            "OK" => {
                tracing::info!(message = "Connected to parent", %parent);
            }
            "DEFERED" => {
                // TODO:
                let addr = connection.peer_addr().unwrap();
                let addr = format!("DEFERED {}", addr);
                tracing::debug!(message = "Parent deferred", %addr);
                return Err("Parent deferred".into());
            }
            _ => {
                tracing::error!(message = "Parent did not respond with OK", %response);
                return Err("Parent did not respond with OK".into());
            }
        }
    } else {
        tracing::error!(message = "Timeout while waiting for response from parent");
        tracing::warn!(message = "Server Starting without parent");
        return Err("Timeout while waiting for response from parent".into());
    }

    Ok(connection)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let (level, span) = if std::option_env!("LOGGER").is_some() {
        (Level::INFO, info_span!("Main"))
    } else {
        (Level::TRACE, trace_span!("Main"))
    };
    tracing_subscriber::fmt().with_max_level(level).init();

    let cfg =
        serde_json::from_str::<config::Config>(include_str!("../config.json")).map_err(|err| {
            tracing::error!("Could not parse config file");
            err
        })?;

    let parent_connection = connect_to_parent(&cfg).await.map_err(|err| {
        tracing::error!(message = "Could not connect to network", %err);
        tracing::warn!(message = "Server Starting without parent");
    });

    let mut parent = None;
    if let Ok(parent_connection) = parent_connection {
        tracing::info!(message = "Connected to parent");
        parent = match parent_connection.peer_addr() {
            Ok(addr) => Some(Client::new(parent_connection, addr)),
            Err(err) => {
                tracing::error!(message = "Could not get parent address", %err);
                tracing::warn!(message = "Server Starting without parent");
                None
            }
        };
    }

    let addr = format!("127.0.0.1:{}", cfg.port());
    let _ = span.enter();

    // Check if the address is already in use
    let addr = std::net::SocketAddr::from_str(&addr).map_err(|err| {
        tracing::error!(message = "Address is in use alread. Set `ADDR` to a different address", %addr, err = %err);
        exit(1);
    }).expect("Should never reach, because of the exit");

    // let addr = match std::net::SocketAddr::from_str(&addr) {
    //     Ok(addr) => addr,
    //     Err(err) => {}
    // };

    let cfg = Arc::new(cfg);
    let (tx, rx) = tokio::sync::mpsc::channel(10);
    let server = crate::server::Server::new(rx, Arc::clone(&cfg), parent).await;
    server.start_daemon().await;

    let connection = tokio::net::TcpListener::bind(addr).await?;
    let addr = connection.local_addr().map_err(|err| {
        tracing::error!(message = "Could not get local address", %err);
        err
    })?;
    tracing::debug!(message = "Listening on", %addr);

    #[cfg(debug_assertions)]
    log_debug_task();

    // Accept incoming connections
    loop {
        if let Ok((stream, addr)) = connection.accept().await {
            let client = crate::client::Client::new(stream, addr);
            tx.clone()
                .send(server::ServerMessages::NewClient(addr, client, tx.clone()))
                .await
                .map_err(|err| tracing::error!(message = "Could not send message to server", %err))
                .unwrap();
        }
    }
}

/// logs the number of active tasks every time it changes.
#[cfg(debug_assertions)]
fn log_debug_task() {
    tokio::spawn(async move {
        use tokio::runtime::Handle;
        let mut last = 0;
        loop {
            let metrics = Handle::current().metrics();

            let n = metrics.active_tasks_count();

            if last != n {
                debug!(message = "Active Task", %n);
            }
            last = n;
        }
    });
}
