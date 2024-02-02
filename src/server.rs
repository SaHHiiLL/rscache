use crate::client::Client;
use core::fmt;
use std::{collections::HashMap, net::SocketAddr};
use tokio::sync::mpsc::{Receiver, Sender};

#[derive(Debug)]
pub struct Server {
    client: HashMap<SocketAddr, Client>,
    rx: Receiver<ServerMessages>,
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
    IncomingMessage(String),
    NewClient(SocketAddr, crate::client::Client, Sender<ServerMessages>),
    RemoveClient(SocketAddr),
    NewClientMessage(crate::message::JoinMessage, SocketAddr),
}

impl Server {
    pub fn new(rx: Receiver<ServerMessages>) -> Self {
        Self {
            client: HashMap::new(),
            rx,
        }
    }

    pub fn start_daemon(mut self) {
        tracing::info!(message = "Starting Server", %self);
        tokio::task::spawn(async move { self.listen_for_messages().await });
    }

    async fn listen_for_messages(&mut self) {
        while let Some(r) = self.rx.recv().await {
            match r {
                ServerMessages::IncomingMessage(_msg) => {}
                ServerMessages::NewClient(addr, client, tx) => {
                    tracing::info!(message = "New client rec", %addr);
                    self.client.insert(addr, client);
                    self.client
                        .get_mut(&addr.clone())
                        .expect("unreachable")
                        .start_client(tx)
                        .await;
                }
                ServerMessages::RemoveClient(addr) => {
                    if self.client.remove(&addr).is_some() {
                        tracing::debug!(message = "removed client at ", %addr);
                    }
                }
                ServerMessages::NewClientMessage(msg, addr) => {
                    // We can be sure that the client as choses from one the CMD
                    if let Some(client) = self.client.get_mut(&addr) {
                        match msg {
                            crate::message::JoinMessage::NodeJoin => {
                                todo!()
                            }
                            crate::message::JoinMessage::Client => {
                                client.lift_probation();
                                client.listen_client_interaction().await;
                            }
                        }
                    }
                }
            }
        }
    }
}
