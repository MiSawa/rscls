use eyre::{Context as _, Result};
use futures::{SinkExt as _, TryStreamExt as _};
use lsp_server::Message;
use tokio::{
    spawn,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};
use tokio_util::codec::{FramedRead, FramedWrite};
use tracing::instrument;

use crate::{
    codec::MessageCodec,
    event::{Event, EventSender},
};

pub struct Client {
    pub sender: UnboundedSender<Message>,
    _handles: [JoinHandle<Result<()>>; 2],
}

impl Client {
    pub fn stdio(event_sender: EventSender) -> Self {
        let (sender, sender_rcv) = unbounded_channel();
        let handle1 = spawn(redirect_stdin(event_sender));
        let handle2 = spawn(redirect_stdout(sender_rcv));
        // TODO: Do something with handles
        Self {
            sender,
            _handles: [handle1, handle2],
        }
    }
}

#[instrument(skip_all)]
async fn redirect_stdout(mut receiver: UnboundedReceiver<Message>) -> Result<()> {
    let mut write = FramedWrite::new(tokio::io::stdout(), MessageCodec);
    while let Some(msg) = receiver.recv().await {
        write
            .send(msg)
            .await
            .wrap_err("Failed to write message to client (=stdout)")?;
    }
    Ok(())
}

#[instrument(skip_all)]
async fn redirect_stdin(sender: EventSender) -> Result<()> {
    let mut read = FramedRead::new(tokio::io::stdin(), MessageCodec);
    while let Some(msg) = read
        .try_next()
        .await
        .wrap_err("Failed to read message from server")?
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
