use std::io::Write;

use eyre::{Context as _, Result};
use lsp_server::Message;
use tokio::{
    spawn,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task::{spawn_blocking, JoinHandle},
};
use tracing::instrument;

use crate::event::{Event, EventSender};

pub struct Client {
    pub sender: UnboundedSender<Message>,
    _handles: [JoinHandle<Result<()>>; 2],
}

impl Client {
    pub fn stdio(event_sender: EventSender) -> Self {
        let (sender, sender_rcv) = unbounded_channel();
        let handle1 = spawn_blocking(|| redirect_stdin(event_sender));
        let handle2 = spawn(redirect_stdout(sender_rcv));
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
async fn redirect_stdout(mut receiver: UnboundedReceiver<Message>) -> Result<()> {
    while let Some(message) = receiver.recv().await {
        let mut out = std::io::stdout().lock();
        message
            .write(&mut out)
            .wrap_err("Failed to write message to client (stdout)")?;
        out.flush()
            .wrap_err("Failed to flush message to client (stdout)")?;
    }
    Ok(())
}
