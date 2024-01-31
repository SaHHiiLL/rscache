use core::fmt;
use std::{io::Write, sync::Arc};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{mpsc::Sender, RwLock},
};

use crate::server::ServerMessages;

#[derive(Debug)]
pub struct Client {
    con: Arc<RwLock<TcpStream>>,
}

impl fmt::Display for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Client")
    }
}

impl Client {
    pub fn new(connection: TcpStream) -> Self {
        let connection = Arc::new(RwLock::new(connection));
        Self { con: connection }
    }

    pub async fn start_client(&mut self, _tx: Sender<ServerMessages>) {
        // Send a welcome message to the client;
        let connection = Arc::clone(&self.con);

        tokio::task::spawn(async move {
            let mut con = connection.as_ref().write().await;
            let welcome_message =
                "\n\nWelcome to Rscache, use `SET` and `GET` for interacting with the database\n\n"
                    .as_bytes();

            let _ = con.write_all(welcome_message).await;

            // READ all messages from the user here
            loop {
                let mut buffer = [0_u8; 8];
                let read = con.read(&mut buffer).await;

                match read {
                    Ok(0) => break,
                    Ok(n) => {
                        // connection is active
                        let msg = &buffer[..n];
                        let msg = String::from_utf8(msg.to_vec());
                        let msg = match msg {
                            Ok(m) => m,
                            Err(err) => {
                                tracing::error!(message = "Error Reading data from the clinet", %err);
                                break;
                            }
                        };
                        println!("{}", msg);
                    }
                    Err(err) => {
                        tracing::error!(message = "Error Reading data from the clinet", %err);
                        break;
                    }
                }
            }
        });
    }

    async fn send_message(&self) {}
}
