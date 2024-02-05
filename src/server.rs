use crate::{
    client::{Client, ClientState},
    database::Database,
    message::{self, ClientMessage},
};
use core::fmt;
use std::{collections::HashMap, net::SocketAddr};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing_subscriber::fmt::format;

#[derive(Debug)]
pub struct Server {
    client: HashMap<SocketAddr, Client>,
    rx: Receiver<ServerMessages>,
    db: Database,
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
    pub async fn new(rx: Receiver<ServerMessages>) -> Self {
        Self {
            client: HashMap::new(),
            rx,
            db: Database::new(),
        }
    }

    pub async fn start_daemon(mut self) {
        tracing::info!(message = "Starting Server", %self);
        tokio::task::spawn(async move { self.listen_for_messages().await });
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
                                // self.db.insert_value(key, msg);
                                // cl.change_state_to_settingkey().await;
                            } else {
                                cl.send_message("Could not parse the messgae".to_string())
                                    .await;
                                continue 'main;
                            }
                        }
                    };

                    match msg {
                        message::ClientMessage::SetKey { key, dur } => {
                            let _ = dur;
                            self.db.insert_key_no_value(key.to_string());
                            cl.change_state_to_settingvalue(key).await;
                        }
                        message::ClientMessage::SetValue { key, value } => {
                            self.db.insert_value(key, value);
                            cl.change_state_to_settingkey().await;
                        }
                        message::ClientMessage::GetValue { key } => {
                            let v = self.db.get(&key);
                            let v = match v {
                                Some(v) => v.to_owned(),
                                None => {
                                    let v = format!("KEY={{{key}}} does not exists");
                                    Some(v)
                                }
                            };

                            match v {
                                Some(v) => cl.send_message(v).await,
                                None => {
                                    let v = format!("KEY={{{key}}} is empty");
                                    cl.send_message(v).await;
                                }
                            }
                        }
                    }
                }
                ServerMessages::NewClient(addr, client, tx) => {
                    tracing::info!(message = "New client rec", %addr);
                    self.client.insert(addr, client);
                    self.client
                        .get_mut(&addr.clone())
                        .expect("unreachable")
                        .keep_open(tx)
                        .await;
                }
                ServerMessages::RemoveClient(addr) => {
                    if self.client.remove(&addr).is_some() {
                        tracing::debug!(message = "removed client at ", %addr);
                    }
                }
            }
        }
    }
}
