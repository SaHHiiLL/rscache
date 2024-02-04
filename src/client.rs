use core::fmt;
use std::{net::SocketAddr, sync::Arc};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::{mpsc::Sender, RwLock},
};
use tracing_subscriber::fmt::format;

use crate::{message, server::ServerMessages};

trait AsyncWritelnExt<S: ToString> {
    async fn writeln(&mut self, msg: S);
}

impl<S> AsyncWritelnExt<S> for TcpStream
where
    S: ToString,
{
    async fn writeln(&mut self, msg: S) {
        let msg = format!("{}\n", msg.to_string());
        self.write_all(msg.as_bytes()).await;
    }
}

impl<S> AsyncWritelnExt<S> for OwnedWriteHalf
where
    S: ToString,
{
    async fn writeln(&mut self, msg: S) {
        let msg = format!("{}\n", msg.to_string());
        self.write_all(msg.as_bytes()).await;
    }
}

#[derive(Debug)]
pub struct Client {
    addr: SocketAddr,
    state: ClientState,
    write: Arc<RwLock<OwnedWriteHalf>>,
    read: Arc<RwLock<OwnedReadHalf>>,
}

// impl Clone for Client {
//     fn clone(&self) -> Self {
//         Self {
//             addr: self.addr.clone(),
//             state: self.state.clone(),
//             write
//         }
//     }
// }

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
        // let con = Arc::new(RwLock::new(connection));
        let (read, write) = connection.into_split();
        let read = Arc::new(RwLock::new(read));
        let write = Arc::new(RwLock::new(write));

        Self {
            addr,
            state: ClientState::Probation,
            write,
            read,
        }
    }

    pub fn lift_probation(&mut self) {
        self.state = ClientState::Free;
    }

    pub async fn keep_open(&mut self, tx: Sender<ServerMessages>) {
        // Send a welcome message to the client;
        let read_h = Arc::clone(&self.read);
        let write_h = Arc::clone(&self.write);
        let addr = self.addr.clone();

        #[allow(clippy::let_underscore_future)]
        let _ = tokio::task::spawn(async move {
            // Will read non stop
            loop {
                let mut buf = [0u8; 64];

                let read = read_h.write().await.read(&mut buf).await;

                match read {
                    Ok(0) => break,
                    Ok(n) => {
                        // Somerthing
                        let msg = &buf[..n];
                        let msg = String::from_utf8(msg.to_vec());
                        let msg = match msg {
                            Ok(m) => m,
                            Err(err) => {
                                tracing::error!(message = "Non-UTF8 string received", %err);
                                return;
                            }
                        };
                        let msg = msg.parse::<message::ClientMessage>();

                        let msg = match msg {
                            Ok(msg) => msg,
                            Err(_) => {
                                // write_h
                                //     .write()
                                //     .await
                                //     .write_all(b"Unable to parse the message\n")
                                //     .await;
                                continue;
                            }
                        };

                        let _ = tx
                            .send(ServerMessages::NewMessage(msg, addr))
                            .await
                            .map_err(|err| {
                                tracing::error!(message = "Could not send message to server", %err);
                            });
                    }
                    Err(_) => break,
                }
            }
            tracing::debug!(message = "Dropping Connection", %addr);
        });
    }

    pub async fn send_message(&mut self, msg: String) {
        let write_h = Arc::clone(&self.write);
        write_h.write().await.writeln(msg).await;
    }
}
