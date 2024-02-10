#![deny(unused_must_use)]
#![allow(clippy::let_underscore_future)]
use std::{str::FromStr, time::Duration};
use tracing::{debug, info, info_span, trace_span, Level};
mod client;
mod database;
mod message;
mod server;

#[tokio::main]
async fn main() {
    let (level, span) = if std::option_env!("LOGGER").is_some() {
        (Level::INFO, info_span!("Main"))
    } else {
        (Level::TRACE, trace_span!("Main"))
    };

    tracing_subscriber::fmt().with_max_level(level).init();

    let addr = std::option_env!("ADDR").unwrap_or("127.0.0.1:6969");
    let _ = span.enter();

    let addr = std::net::SocketAddr::from_str(addr).map_err(|err| {
        tracing::error!(message = "Address is in use alread. Set `ADDR` to a different address", %addr);
        err
    }).unwrap();

    let (tx, rx) = tokio::sync::mpsc::channel(10);
    let server = crate::server::Server::new(rx).await;
    server.start_daemon().await;

    let connection = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::info!(message = "Listening on", %addr);

    #[cfg(debug_assertions)]
    {
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
                tokio::time::sleep(Duration::from_secs(20)).await;
            }
        });
    }

    loop {
        if let Ok((stream, addr)) = connection.accept().await {
            let client = crate::client::Client::new(stream, addr);
            let _ = tx
                .clone()
                .send(server::ServerMessages::NewClient(addr, client, tx.clone()))
                .await
                .map_err(|err| tracing::error!(message = "Could not send message to server", %err));
        }
    }
}
