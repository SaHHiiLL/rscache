use std::{str::FromStr, time::Duration};

use tracing::Level;
mod client;
mod database;
mod message;
mod server;

#[allow(dead_code)]
#[tokio::main]
async fn main() {
    let level = if std::option_env!("LOGGER").is_some() {
        Level::INFO
    } else {
        Level::TRACE
    };

    tracing_subscriber::fmt()
        // all spans/events with a level higher than TRACE (e.g, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(level)
        // sets this to be the default, global collector for this application.
        .init();

    let addr = std::option_env!("ADDR").unwrap_or("127.0.0.1:6969");

    let addr = std::net::SocketAddr::from_str(addr).map_err(|err| {
        tracing::error!(message = "Address is in use alread. Set `ADDR` to a different address", %addr);
        err
    }).unwrap();

    let (tx, rx) = tokio::sync::mpsc::channel(10);
    let server = crate::server::Server::new(rx);
    server.start_daemon();

    let connection = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::info!(message = "Listening on", %addr);

    #[cfg(debug_assertions)]
    {
        tokio::spawn(async move {
            use tokio::runtime::Handle;
            loop {
                let metrics = Handle::current().metrics();

                let n = metrics.active_tasks_count();

                tracing::debug!(message = "Active Task", %n);
                tokio::time::sleep(Duration::from_secs(5)).await;
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
