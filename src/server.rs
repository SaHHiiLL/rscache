use crate::{
    client::{Client, ClientState},
    config::Config,
    database::Database,
    message::{self, ClientMessage},
};
use core::fmt;
use std::{collections::HashMap, net::SocketAddr, process::exit, sync::Arc};
use tokio::sync::{
    mpsc::{Receiver, Sender},
    RwLock,
};
use tracing::debug_span;

#[derive(Debug)]
pub struct Server {
    client: HashMap<SocketAddr, Client>,
    rx: Receiver<ServerMessages>,
    db: Database,
    config: Arc<Config>,
}

impl Drop for Server {
    fn drop(&mut self) {
        tracing::debug!("Dropping Server");
        let futures = self.client.iter().map(|(addr, client)| async move {
            let span = debug_span!("Dropping Server");
            let _graurd = span.enter();
            tracing::info!(message = "Disconnecting", address = %addr);
            client.disconnect().await;
        });
        let _ = futures::future::join_all(futures);
        exit(0);
    }
}

impl fmt::Display for Server {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let map = self
            .client
            .iter()
            .map(|f| format!("{} : {}", f.0, f.1).to_string())
            .collect::<Vec<String>>()
            .join(",");

        write!(f, "Server {{ client: {}, rx: Hidden }}", map)
    }
}

#[derive(Debug)]
pub enum ServerMessages {
    NewMessage(String, SocketAddr),
    NewClient(SocketAddr, crate::client::Client, Sender<ServerMessages>),
    RemoveClient(SocketAddr),
}

impl Server {
    pub async fn new(rx: Receiver<ServerMessages>, config: Arc<Config>) -> Self {
        let db = Database::new(Arc::clone(&config));
        Self {
            client: HashMap::new(),
            rx,
            db,
            config,
        }
    }

    pub async fn start_daemon(mut self) {
        tracing::debug!(message = "Starting Server", %self);
        self.db.keep_valid().await;
        tokio::task::spawn(async move { self.listen_for_messages().await });
        // tokio::task::spawn(async move { keep_database_valid(db).await });
    }

    async fn listen_for_messages(&mut self) {
        'main: while let Some(r) = self.rx.recv().await {
            match r {
                ServerMessages::NewMessage(msg, addr) => {
                    let cl = self.client.get_mut(&addr);
                    if cl.is_none() {
                        continue 'main;
                    }

                    let cl = cl.expect("Client should be in map");

                    let clm = msg.parse::<ClientMessage>();
                    let msg = match clm {
                        Ok(msg) => msg,
                        Err(_) => {
                            if let ClientState::SettingValue { key } = cl.get_state().await {
                                ClientMessage::SetValue { key, value: msg }
                            } else {
                                cl.send_messageln("Could not parse the messgae".to_string())
                                    .await;
                                continue 'main;
                            }
                        }
                    };

                    match msg {
                        message::ClientMessage::SetKey { key, dur } => {
                            self.db.insert_key(key.to_string(), dur).await;
                            cl.change_state_to_settingvalue(key).await;
                        }
                        message::ClientMessage::SetValue { key, value } => {
                            self.db.insert_key_value(key, value).await;
                            tracing::debug!("Set Value");
                            cl.change_state_to_settingkey().await;
                        }
                        message::ClientMessage::GetValue { key } => {
                            let v = self.db.get_or_remove(key.to_string()).await;
                            let v = match v {
                                Some(v) => v.to_owned().inner(),
                                None => {
                                    let v = format!("KEY={{{}}} does not exists\n", key);
                                    Some(v)
                                }
                            };

                            match v {
                                Some(v) => cl.send_message(v).await,
                                None => {
                                    let v = format!("KEY={{{key}}} is empty");
                                    cl.send_messageln(v).await;
                                }
                            }
                        }
                    }
                }
                ServerMessages::NewClient(addr, client, tx) => {
                    tracing::debug!(message = "New client", %addr);
                    self.client.insert(addr, client);
                    self.client
                        .get_mut(&addr.clone())
                        .expect("unreachable")
                        .keep_open(tx)
                        .await;
                }
                ServerMessages::RemoveClient(addr) => {
                    if let Some(client) = self.client.remove(&addr) {
                        client.disconnect().await;
                        tracing::debug!(message = "removed client at ", %addr);
                    }
                }
            }
        }
    }
}
