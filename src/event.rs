use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc::{sync_channel, Receiver, SendError, SyncSender},
    Arc,
};

use lsp_server::Message;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(usize);

pub enum Event {
    ClientToServer(Message),
    ServerToClient(Message),
    ServerLog(String),
    NeedReload(Version),
}

pub type EventReceiver = Receiver<Event>;

#[derive(Clone)]
pub struct EventSender {
    sender: SyncSender<Event>,
    version: Arc<AtomicUsize>,
}

impl EventSender {
    fn new(sender: SyncSender<Event>) -> Self {
        Self {
            sender,
            version: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn send(&self, event: Event) -> Result<(), SendError<Event>> {
        self.sender.send(event)
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
    let (sender, receiver) = sync_channel(256);
    (EventSender::new(sender), receiver)
}
