#![deny(unused_must_use)]
#![allow(clippy::let_underscore_future)]
use std::{net::SocketAddr, process::exit, str::FromStr};
use tracing::{debug, info_span, trace_span, Level};
mod client;
mod config;
mod database;
mod message;
mod server;

use clap::Parser;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    join: Option<String>,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let (level, span) = if std::option_env!("LOGGER").is_some() {
        (Level::INFO, info_span!("Main"))
    } else {
        (Level::TRACE, trace_span!("Main"))
    };
    tracing_subscriber::fmt().with_max_level(level).init();

    let args = Args::parse();

    let cfg =
        serde_json::from_str::<config::Config>(include_str!("../config.json")).map_err(|err| {
            tracing::error!("Could not parse config file");
            err
        })?;
    if args.join.is_some() {
        // make a tcp connection
        let addr = args.join.unwrap();
        let addr = format!("{}:{}", addr, cfg.port());
        let addr = dbg!(addr).parse::<SocketAddr>().unwrap();
        tokio::net::TcpStream::connect(addr).await.unwrap();
    }

    let addr = format!("127.0.0.1:{}", cfg.port());
    let _ = span.enter();

    let addr = std::net::SocketAddr::from_str(&addr).map_err(|err| {
        tracing::error!(message = "Address is in use alread. Set `ADDR` to a different address", %addr, err = %err);
        exit(1);
    }).unwrap();

    let (tx, rx) = tokio::sync::mpsc::channel(10);
    let server = crate::server::Server::new(rx).await;
    server.start_daemon().await;

    let connection = tokio::net::TcpListener::bind(addr).await?;
    tracing::debug!(message = "Listening on", %addr);

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
            }
        });
    }

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
