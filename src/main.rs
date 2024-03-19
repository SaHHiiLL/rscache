#![deny(unused_must_use)]
#![allow(clippy::let_underscore_future)]
use std::{net::SocketAddr, process::exit, str::FromStr, sync::Arc, time::Duration};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, info_span, trace_span, Level};

use crate::client::Client;
mod client;
mod config;
mod database;
mod message;
mod server;

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

    let mut parent = None;
    //
    // If the parent is set, then we need to connect to the parent and send a message to the parent
    // to join the network. The parent will then respond with an "OK" message. If the parent does
    // not respond with an "OK" message, then we should exit the program.
    // TIMEOUT is set to 10 seconds.
    //
    // NOTE:
    // If the parent responds with "DEFERED <addr>:<port>", then connect the defered parent and
    // abandon the current parent.
    if cfg.parent().is_some() {
        let parent_c = cfg.parent().unwrap().into();
        // make a TCP connection with the parent and wait till the respose is "OK";
        let mut stream = tokio::net::TcpStream::connect(parent_c)
            .await
            .map_err(|err| {
                tracing::error!(message = "Could not connect to parent", %err);
                err
            })?;
        let addrs = stream.local_addr().map_err(|err| {
            tracing::error!(message = "Could not get local address", %err);
            err
        })?;
        let msg = format!("JOIN {}", addrs);
        stream.write_all(msg.as_bytes()).await?;

        let mut buf = [0; 1024];
        if let Ok(n) = tokio::time::timeout(Duration::from_secs(10), stream.read(&mut buf)).await {
            let n = n.map_err(|err| {
                tracing::error!(message = "Could not read from parent", %err);
                err
            })?;
            let response = std::str::from_utf8(&buf[..n])
                .map_err(|err| {
                    tracing::error!(message = "Could not parse response", %err);
                    err
                })
                .unwrap();

            match response {
                "OK" => {
                    let addr = stream.peer_addr().unwrap();
                    parent = Some(Client::new(stream, addr));
                }
                "DEFERED" => {
                    let addr = stream.peer_addr().unwrap();
                    let addr = format!("DEFERED {}", addr);
                    tracing::debug!(message = "Parent deferred", %addr);
                    exit(1);
                }
                _ => {
                    tracing::error!(message = "Parent did not respond with OK", %response);
                    exit(1);
                }
            }
        } else {
            tracing::error!(message = "Timeout while waiting for response from parent");
            tracing::warn!(message = "Server Starting without parent");
        }
    }

    let addr = format!("127.0.0.1:{}", cfg.port());
    let _ = span.enter();

    // Check if the address is already in use
    let addr = std::net::SocketAddr::from_str(&addr).map_err(|err| {
        tracing::error!(message = "Address is in use alread. Set `ADDR` to a different address", %addr, err = %err);
        exit(1);
    }).expect("Should never reach, because of the exit");

    let cfg = Arc::new(cfg);
    let (tx, rx) = tokio::sync::mpsc::channel(10);
    let server = crate::server::Server::new(rx, Arc::clone(&cfg), parent).await;
    server.start_daemon().await;

    let connection = tokio::net::TcpListener::bind(addr).await?;
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
