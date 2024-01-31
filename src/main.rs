use std::str::FromStr;

use tracing::Level;
mod client;
mod server;

#[allow(dead_code)]
#[tokio::main]
async fn main() {
    let level = if let Some(_) = std::option_env!("LOGGER") {
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

    loop {
        if let Ok((stream, addr)) = connection.accept().await {
            let client = crate::client::Client::new(stream);
            let _ = tx
                .clone()
                .send(server::ServerMessages::NewClient(addr, client, tx.clone()))
                .await
                .map_err(|err| tracing::error!(message = "Could not send message to server", %err));
        }
    }
}
