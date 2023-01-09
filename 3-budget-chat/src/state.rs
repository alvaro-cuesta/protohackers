use std::collections::HashMap;

use tokio::sync::mpsc;

use crate::{ClientName, Message};

#[derive(Debug, Default)]
pub struct State {
    clients: HashMap<ClientName, mpsc::UnboundedSender<Event>>,
}

impl State {
    pub fn add_client(
        &mut self,
        name: ClientName,
    ) -> anyhow::Result<mpsc::UnboundedReceiver<Event>> {
        if self.clients.contains_key(&name) {
            return Err(anyhow::Error::msg("Name already taken"));
        }

        let event = Event::NewClient(name.clone());

        for sender in self.clients.values() {
            // Okay to ignore errors, just drop them
            let _ = sender.send(event.clone());
        }

        let (send, receive) = mpsc::unbounded_channel();
        self.clients.insert(name, send);

        Ok(receive)
    }

    pub fn get_present_names(&self) -> Vec<ClientName> {
        self.clients.keys().cloned().collect()
    }

    pub fn broadcast_message(&mut self, from: ClientName, message: Message) {
        let event = Event::Message(from.clone(), message);

        for (name, sender) in self.clients.iter() {
            if name == &from {
                continue;
            }

            // Okay to ignore errors, just drop them
            let _ = sender.send(event.clone());
        }
    }

    pub fn disconnect_client(&mut self, name: ClientName) {
        self.clients.remove(&name);

        let event = Event::Disconnect(name.clone());

        for (receiver_name, sender) in self.clients.iter() {
            if receiver_name == &name {
                continue;
            }

            // Okay to ignore errors, just drop them
            let _ = sender.send(event.clone());
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    NewClient(ClientName),
    Message(ClientName, Message),
    Disconnect(ClientName),
}
