use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use lsp_server::Message;
use tokio::sync::mpsc::{error::SendError, unbounded_channel, UnboundedReceiver, UnboundedSender};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(usize);

#[derive(Debug)]
pub enum Event {
    ClientToServer(Message),
    ServerToClient(Message),
    ServerLog(String),
    NeedReload(Version),
}

pub type EventReceiver = UnboundedReceiver<Event>;

#[derive(Clone)]
pub struct EventSender {
    sender: UnboundedSender<Event>,
    version: Arc<AtomicUsize>,
}

impl EventSender {
    fn new(sender: UnboundedSender<Event>) -> Self {
        Self {
            sender,
            version: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn send(&self, event: Event) -> Result<(), Box<SendError<Event>>> {
        self.sender.send(event).map_err(Box::new)
    }

    pub fn mark_need_reload(&self) {
        let version = self.version.load(Ordering::SeqCst);
        self.sender.send(Event::NeedReload(Version(version))).ok();
    }

    pub fn start_reload(&self) -> Version {
        self.version.fetch_add(1, Ordering::SeqCst);
        Version(self.version.load(Ordering::SeqCst))
    }

    pub fn current_version(&self) -> Version {
        Version(self.version.load(Ordering::SeqCst))
    }
}

pub fn new_event_bus() -> (EventSender, EventReceiver) {
    let (sender, receiver) = unbounded_channel();
    (EventSender::new(sender), receiver)
}
