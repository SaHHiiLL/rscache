use crate::{client::Client, database::Database, message};
use core::fmt;
use std::{collections::HashMap, net::SocketAddr};
use tokio::sync::mpsc::{Receiver, Sender};

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
    NewMessage(message::ClientMessage, SocketAddr),
    IncomingMessage(String),
    NewClient(SocketAddr, crate::client::Client, Sender<ServerMessages>),
    RemoveClient(SocketAddr),
    NewClientMessage(
        crate::message::JoinMessage,
        SocketAddr,
        Sender<ServerMessages>,
    ),
}

impl Server {
    pub async fn new(rx: Receiver<ServerMessages>) -> Self {
        Self {
            client: HashMap::new(),
            rx,
            db: Database::new().await,
        }
    }

    pub fn start_daemon(mut self) {
        tracing::info!(message = "Starting Server", %self);
        tokio::task::spawn(async move { self.listen_for_messages().await });
    }

    async fn listen_for_messages(&mut self) {
        while let Some(r) = self.rx.recv().await {
            match r {
                ServerMessages::NewMessage(msg, addr) => match msg {
                    message::ClientMessage::SetKey { key, dur } => {
                        self.db.insert_key_no_value(key);
                    }
                    message::ClientMessage::SetValue { key, value } => {
                        self.db.insert_value(key, value)
                    }
                    message::ClientMessage::GetValue { key } => {
                        let v = self.db.get(&key).unwrap();
                        let v = match v {
                            Some(d) => d.to_owned(),
                            None => String::from("lol"),
                        };
                        tracing::debug!(v);

                        let cl = self.client.get_mut(&addr);

                        if let Some(cl) = cl {
                            cl.send_message(v).await;
                        }
                    }
                },
                ServerMessages::IncomingMessage(_msg) => {}
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
                ServerMessages::NewClientMessage(msg, addr, tx) => {
                    // We can be sure that the client as choses from one the CMD
                    if let Some(client) = self.client.get_mut(&addr) {
                        match msg {
                            crate::message::JoinMessage::NodeJoin => {
                                todo!()
                            }
                            crate::message::JoinMessage::Client => {
                                client.lift_probation();
                                // client.listen_client_interaction(tx.clone()).await;
                            }
                        }
                    }
                }
            }
        }
    }
}
