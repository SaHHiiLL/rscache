use core::fmt;
use std::{sync::Arc, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{mpsc::Sender, RwLock},
    time::timeout,
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

    pub async fn start_client(&mut self, tx: Sender<ServerMessages>) {
        // Send a welcome message to the client;
        let connection = Arc::clone(&self.con);

        let _ = tokio::task::spawn(async move {
            let duration = Duration::from_secs(5);
            let welcome_message = r"

Welcome to Rscache, use `SET` and `GET` for interacting with the database

Rscache uses `Slashcommands`

Please send one of the following message. Timeout is 5 seconds
    - /NODEJOIN
    - /Client
    - /HELP <name of the command>
"
            .as_bytes();

            let _ = connection
                .as_ref()
                .write()
                .await
                .write_all(welcome_message)
                .await;

            let res = timeout(
                duration,
                Client::read_once(Arc::clone(&connection), tx.clone()),
            )
            .await;

            // waits for the future to complete in the duration -> if the future is completed in
            // the given time, OK branch is executed - if the future is not executed in the given
            // time Err branch is executed
            match res {
                Ok(_) => {
                    println!("Connection still active");
                }
                Err(_) => {
                    let mut con = connection.write().await;
                    let bye_msg = "\n Connection Timeout\n\n".as_bytes();
                    let _ = con.write_all(bye_msg).await;
                    let addr = con.peer_addr().unwrap();
                    con.shutdown().await.unwrap();
                    let _ = tx
                        .send(ServerMessages::RemoveClient(addr))
                        .await
                        .map_err(|err| {
                            tracing::error!(message = "Could not send message to server", %err);
                        });
                    println!("Connection not active");
                }
            }
        });
    }

    async fn read_once(con: Arc<RwLock<TcpStream>>, tx: Sender<ServerMessages>) {
        let mut buffer = [0_u8; 8];
        let mut con = con.as_ref().write().await;
        let read = con.read(&mut buffer).await;

        // READ's once
        loop {
            match read {
                Ok(0) => {
                    break;
                }
                Ok(n) => {
                    // connection is active
                    let msg = &buffer[..n];
                    let msg = String::from_utf8(msg.to_vec());
                    let msg = match msg {
                        Ok(m) => m,
                        Err(err) => {
                            tracing::error!(message = "Error Reading data from the clinet", %err);
                            return;
                        }
                    };

                    let _ = tx
                        .send(ServerMessages::IncomingMessage(msg))
                        .await
                        .map_err(|err| {
                            tracing::error!(message = "Could not send message to server", %err);
                        });
                }
                Err(_) => {
                    tracing::error!(message = "Error Reading data from the clinet");
                    break;
                }
            };
        }
    }
}
