use std::sync::mpsc::{channel, Receiver, Sender};

use lsp_server::Message;

pub enum Event {
    ClientToServer(Message),
    ServerToClient(Message),
    ServerLog(String),
}

pub type EventSender = Sender<Event>;
pub type EventReceiver = Receiver<Event>;

pub fn new_event_bus() -> (EventSender, EventReceiver) {
    channel()
}
