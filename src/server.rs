use std::ffi::OsStr;

use eyre::{Context as _, Result};
use futures::{sink::SinkExt as _, stream::TryStreamExt as _};
use lsp_server::Message;
use tokio::{
    io::{AsyncBufReadExt as _, BufReader},
    process::{Child, ChildStderr, ChildStdin, ChildStdout},
    spawn,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};
use tokio_util::codec::{FramedRead, FramedWrite};
use tracing::instrument;

use crate::{
    codec::MessageCodec,
    event::{Event, EventSender},
    lsp_extra::MessageExt as _,
};

pub struct Server {
    #[allow(unused)]
    process: Child,
    pub sender: UnboundedSender<Message>,
    _handles: [JoinHandle<Result<()>>; 3],
}

impl Server {
    pub fn spawn(event_sender: EventSender, path: impl AsRef<OsStr>) -> Result<Self> {
        let mut process = tokio::process::Command::new(path)
            .kill_on_drop(true)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .wrap_err("failed to sapwn the server")?;

        let (sender, sender_rcv) = unbounded_channel();

        let event_sender_clone = event_sender.clone();
        let stderr = process.stderr.take().unwrap();
        let stdin = process.stdin.take().unwrap();
        let stdout = process.stdout.take().unwrap();
        let handle1 = spawn(redirect_log(event_sender_clone, stderr));
        let handle2 = spawn(redirect_send(sender_rcv, stdin));
        let handle3 = spawn(redirect_receive(event_sender, stdout));

        // TODO: Do something with handles
        Ok(Self {
            process,
            sender,
            _handles: [handle1, handle2, handle3],
        })
    }
}

#[instrument(skip_all)]
async fn redirect_log(sender: EventSender, child_log: ChildStderr) -> Result<()> {
    let mut lines = BufReader::new(child_log).lines();
    while let Some(line) = lines.next_line().await? {
        if sender.send(Event::ServerLog(line)).is_err() {
            break;
        }
    }
    Ok(())
}

#[instrument(skip_all)]
async fn redirect_send(mut receiver: UnboundedReceiver<Message>, stdin: ChildStdin) -> Result<()> {
    let mut write = FramedWrite::new(stdin, MessageCodec);
    while let Some(msg) = receiver.recv().await {
        let need_exit = msg.is_exit();
        write
            .send(msg)
            .await
            .wrap_err("Failed to write message to server")?;
        if need_exit {
            tracing::info!("Exit loop sending message to server");
            break;
        }
    }
    Ok(())
}

#[instrument(skip_all)]
async fn redirect_receive(sender: EventSender, stdout: ChildStdout) -> Result<()> {
    let mut read = FramedRead::new(stdout, MessageCodec);
    while let Some(msg) = read
        .try_next()
        .await
        .wrap_err("Failed to read message from server")?
    {
        if sender.send(Event::ServerToClient(msg)).is_err() {
            break;
        }
    }
    Ok(())
}
