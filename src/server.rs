use crate::{
    client::{Client, ClientState},
    config::Config,
    database::Database,
    message::{self, ClientMessage},
};
use core::fmt;
use std::{collections::HashMap, net::SocketAddr, process::exit, rc::Rc, sync::Arc};
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
    nodes: Vec<Client>,
    parent: Option<Client>,
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

#[derive(Debug)]
struct Error {
    message: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Server {
    pub async fn new(
        rx: Receiver<ServerMessages>,
        config: Arc<Config>,
        parent: Option<Client>,
    ) -> Self {
        let db = Database::new(Arc::clone(&config));
        Self {
            client: HashMap::new(),
            rx,
            db,
            config,
            nodes: Vec::new(),
            parent,
        }
    }

    pub async fn start_daemon(mut self) {
        tracing::debug!(message = "Starting Server", %self);
        self.db.keep_valid().await;
        tokio::task::spawn(async move { self.listen_for_messages().await });
    }

    // TODO: deperecated
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
                        message::ClientMessage::JoinNode { addr } => {
                            match self.client.remove(&addr) {
                                Some(client) => {
                                    let res = self.add_node(client).await;
                                    match res {
                                        Ok(client) => {
                                            client.send_messageb(b"OK").await;
                                        }
                                        Err(op_client) => {
                                            if let Some(client) = op_client {
                                                self.client.insert(addr, client);
                                            } else {
                                                unreachable!("This should not happen: There should always be a last client");
                                            }
                                        }
                                    }
                                }
                                None => {
                                    tracing::error!(message = "Client not found", %addr);
                                    tracing::error!(message = "Clinet was able to send a message over TCP and pretened as a Client", %addr);
                                    tracing::debug!(message = "FAILED TO JOIN AS NODE", %addr);
                                }
                            }
                        }
                        message::ClientMessage::Defered { addr } => {
                            // TODO: Defer the connection to the next node in the list
                            tracing::debug!(message = "Defered", %addr);
                        }
                    }
                }
                ServerMessages::NewClient(addr, client, tx) => {
                    tracing::debug!(message = "New client", %addr);
                    let mut client = self.client.insert(addr, client);
                    if let Some(ref mut client) = client {
                        client.keep_open(tx).await;
                    }
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

    /// Adds a new node to the topolgy if and only if the max_nodes is not reached. If the max_nodes
    /// is reached, the trys to find the next node in line that can accept the connection.
    pub async fn add_node(&mut self, client: Client) -> Result<&mut Client, Option<Client>> {
        todo!()
    }

    async fn ask_deferred_node(&mut self, client: Client) -> Option<Client> {
        todo!()
    }

    /// Since the topology is a tree, we can drop the connection to the next node in line - starts
    /// from index 0 and goes up to the max_nodes. the client will always be part of the connection
    /// list as it's not possible to have a maxed out connection list.
    ///
    /// This function works like a pyramid scheme, where the last node in the list is the one that
    /// gets the connection.
    pub async fn drop_connection_to_nodes(&mut self, client: Client) {}
}
