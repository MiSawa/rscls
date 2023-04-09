use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

use eyre::{Context as _, Result};
use lsp_server::Message;
use tracing::instrument;

use crate::event::{Event, EventSender};

pub struct Client {
    pub sender: SyncSender<Message>,
    _handles: [std::thread::JoinHandle<Result<()>>; 2],
}

impl Client {
    pub fn stdio(event_sender: EventSender) -> Self {
        let (sender, sender_rcv) = sync_channel(0);
        let handle1 = std::thread::spawn(|| redirect_stdin(event_sender));
        let handle2 = std::thread::spawn(|| redirect_stdout(sender_rcv));
        // TODO: Do something with handles
        Self {
            sender,
            _handles: [handle1, handle2],
        }
    }
}

#[instrument(skip_all)]
fn redirect_stdin(sender: EventSender) -> Result<()> {
    let mut stdin = std::io::stdin().lock();
    while let Some(msg) =
        Message::read(&mut stdin).wrap_err("Failed to read message from client (stdin)")?
    {
        use lsp_types::notification::{Exit, Notification as _};
        let need_exit = matches!(&msg, Message::Notification(notification) if notification.method == Exit::METHOD);
        sender.send(Event::ClientToServer(msg)).unwrap();
        if need_exit {
            tracing::info!("Exit loop receiving message from client");
            break;
        }
    }
    Ok(())
}

#[instrument(skip_all)]
fn redirect_stdout(receiver: Receiver<Message>) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    receiver
        .into_iter()
        .try_for_each(|it| it.write(&mut stdout))
        .wrap_err("Failed to write message to client (stdout)")
}
