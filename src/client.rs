use core::fmt;
use std::{net::SocketAddr, sync::Arc, time::Duration};

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
    addr: SocketAddr,
    state: ClientState,
}

impl Clone for Client {
    fn clone(&self) -> Self {
        Self {
            con: Arc::clone(&self.con),
            addr: self.addr.clone(),
            state: self.state.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ClientState {
    Probation,
    Free,
}

impl fmt::Display for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Client")
    }
}

impl Client {
    pub fn new(connection: TcpStream, addr: SocketAddr) -> Self {
        let con = Arc::new(RwLock::new(connection));
        Self {
            con,
            addr,
            state: ClientState::Probation,
        }
    }

    pub fn lift_probation(&mut self) {
        self.state = ClientState::Free;
    }

    pub async fn start_client(&mut self, tx: Sender<ServerMessages>) {
        // Send a welcome message to the client;
        let connection = Arc::clone(&self.con);
        let addr = self.addr.clone();

        #[allow(clippy::let_underscore_future)]
        let _ = tokio::task::spawn(async move {
            let duration = Duration::from_secs(5);
            let welcome_message = "

Welcome to Rscache.

Please send one of the following message. Timeout is 5 seconds
    - NODEJOIN
    - Client
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
                Client::read_once(Arc::clone(&connection), tx.clone(), addr),
            )
            .await;

            // waits for the future to complete in the duration -> if the future is completed in
            // the given time, OK branch is executed - if the future is not executed in the given
            // time Err branch is executed

            match res {
                Ok(true) => {
                    // Keep the connection alive for future
                }
                Ok(false) => {
                    Client::disconnect(Arc::clone(&connection), tx.clone(), "").await;
                }
                _ => {
                    Client::disconnect(
                        Arc::clone(&connection),
                        tx.clone(),
                        "\n\nConnection Timeout\n\n",
                    )
                    .await;
                }
            }
        });
    }

    pub async fn listen_client_interaction(&mut self, tx: Sender<ServerMessages>) {
        self.write_once(
            "\n\nYou are connected... use `SET` or `GET` for interacting with the database\n",
        )
        .await;
        let msg = "\n\nUse `SET <KEY> <TIME_TO_LIVE>` Your next message will be considered as a the VALUE to the above, you will have 10MINUTES to Enter your value. a new line char is considered as the EOF if no EOF is provided before\n";

        self.write_once(msg).await;

        let con = Arc::clone(&self.con);
        tokio::task::spawn(async move { Client::read(con, tx) });
    }

    async fn read(con: Arc<RwLock<TcpStream>>, tx: Sender<ServerMessages>) -> bool {
        loop {
            let mut buffer = [0u8; 64];

            let read = con.write().await.read(&mut buffer).await;
            match read {
                Ok(0) => return true,
                Ok(n) => {
                    let msg = &buffer[..n];
                    let msg = String::from_utf8(msg.to_vec());
                    let msg = match msg {
                        Ok(m) => m,
                        Err(err) => {
                            tracing::error!(message = "Error Reading data from the clinet", %err);
                            return false;
                        }
                    };
                    let msg = msg.parse();
                }
                Err(_) => {
                    tracing::error!("Could not read from client");
                    break;
                }
            };
        }
        false
    }

    async fn write_once<T: Into<String>>(&mut self, msg: T) {
        let msg = msg.into().as_bytes().to_vec();
        let _ = self.con.write().await.write_all(&msg).await;
    }

    async fn disconnect<S: Into<String>>(
        con: Arc<RwLock<TcpStream>>,
        tx: Sender<ServerMessages>,
        bye_msg: S,
    ) {
        let bye_msg = bye_msg.into();
        let mut con = con.write().await;

        // Only write if the message contains something
        if bye_msg != "" {
            let bye_msg = bye_msg.as_bytes().to_vec();
            let _ = con.write_all(&bye_msg).await;
        }

        let addr = con.peer_addr().unwrap();
        con.shutdown().await.unwrap();

        // Send message to server so it can remove client from map
        let _ = tx
            .send(ServerMessages::RemoveClient(addr))
            .await
            .map_err(|err| {
                tracing::error!(message = "Could not send message to server", %err);
            });
    }

    // TODO: rename this to something like read until verification
    async fn read_once(
        con: Arc<RwLock<TcpStream>>,
        tx: Sender<ServerMessages>,
        addr: SocketAddr,
    ) -> bool {
        let mut buffer = [0u8; 32];
        let mut con = con.as_ref().write().await;
        let read = con.read(&mut buffer).await;

        // READ's once
        match read {
            Ok(0) => {}
            Ok(n) => {
                // connection is active
                let msg = &buffer[..n];
                let msg = String::from_utf8(msg.to_vec());
                let msg = match msg {
                    Ok(m) => m,
                    Err(err) => {
                        tracing::error!(message = "Error Reading data from the clinet", %err);
                        return false;
                    }
                };

                // Else let the connection drop through
                if let Ok(msg) = msg.parse() {
                    let _ = tx
                        .send(ServerMessages::NewClientMessage(msg, addr, tx.clone()))
                        .await
                        .map_err(|err| {
                            tracing::error!(message = "Could not send message to server", %err);
                        });
                    return true;
                }
                tracing::debug!(msg);
                let msg = "Incorrect CMD, dropping connection\n".as_bytes();
                let _ = con.write_all(msg).await;
            }
            Err(_) => {
                tracing::error!(message = "Error Reading data from the clinet");
            }
        };

        false
    }
}
