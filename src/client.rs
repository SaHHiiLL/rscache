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

use crate::server::ServerMessages;

trait AsyncWritelnExt<S: ToString> {
    async fn writeln(&mut self, msg: S);
}

impl<S> AsyncWritelnExt<S> for TcpStream
where
    S: ToString,
{
    async fn writeln(&mut self, msg: S) {
        let msg = format!("{}\n", msg.to_string());
        let _ = self.write_all(msg.as_bytes()).await;
    }
}

impl<S> AsyncWritelnExt<S> for OwnedWriteHalf
where
    S: ToString,
{
    async fn writeln(&mut self, msg: S) {
        let msg = format!("{}\n", msg.to_string());
        let _ = self.write_all(msg.as_bytes()).await;
    }
}

#[derive(Debug)]
pub struct Client {
    addr: SocketAddr,
    state: Arc<RwLock<ClientState>>,
    write: Arc<RwLock<OwnedWriteHalf>>,
    read: Arc<RwLock<OwnedReadHalf>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientState {
    SettingValue { key: String },
    SettingKey,
}

impl ClientState {
    fn setting_key(&mut self) {
        *self = ClientState::SettingKey;
    }

    fn setting_value(&mut self, key: String) {
        *self = ClientState::SettingValue { key }
    }
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
        let state = Arc::new(RwLock::new(ClientState::SettingKey));

        Self {
            addr,
            state,
            write,
            read,
        }
    }

    pub async fn keep_open(&mut self, tx: Sender<ServerMessages>) {
        // Send a welcome message to the client;
        let read_h = Arc::clone(&self.read);
        let addr = self.addr;

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
                                break;
                            }
                        };

                        let _ = tx
                            .send(ServerMessages::NewMessage(msg, addr))
                            .await
                            .map_err(|err| {
                                tracing::error!(message = "Could not send message to server -> NewMessage", %err);
                            });
                    }
                    Err(_) => break,
                }
            }
            tracing::debug!(message = "Dropping Connection", %addr);
            let _ = tx
                .send(ServerMessages::RemoveClient(addr))
                .await
                .map_err(|err| {
                    tracing::error!(
                        message = "Could not send message to server -> Disconnect Message", %err
                    )
                });
        });
    }

    pub async fn disconnect(&self) {
        let _ = self.write.write().await.shutdown().await;
    }

    pub async fn get_state(&self) -> ClientState {
        Arc::clone(&self.state).read().await.to_owned()
    }

    pub async fn send_message(&mut self, msg: String) {
        let write_h = Arc::clone(&self.write);
        let _ = write_h.write().await.write(msg.as_bytes()).await;
    }
    pub async fn send_messageln(&mut self, msg: String) {
        let write_h = Arc::clone(&self.write);
        write_h.write().await.writeln(msg).await;
    }

    pub async fn change_state_to_settingkey(&mut self) {
        Arc::clone(&self.state).write().await.setting_key();
    }
    pub async fn change_state_to_settingvalue(&mut self, key: String) {
        Arc::clone(&self.state).write().await.setting_value(key)
    }
}
