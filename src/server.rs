use std::{
    ffi::OsStr,
    io::{BufRead, BufReader, BufWriter},
    sync::mpsc::{sync_channel, Receiver, SyncSender},
};

use eyre::{Context as _, Result};
use lsp_server::Message;
use tracing::instrument;

use crate::event::{Event, EventSender};

pub struct Server {
    #[allow(unused)]
    process: ChildGuard,
    pub sender: SyncSender<Message>,
    _handles: [std::thread::JoinHandle<Result<()>>; 3],
}

struct ChildGuard(std::process::Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Err(e) = self.0.kill() {
            if e.kind() != std::io::ErrorKind::InvalidInput {
                tracing::error!(?e, "Failed to kill server on drop");
            }
        }
    }
}

impl Server {
    pub fn spawn(event_sender: EventSender, path: impl AsRef<OsStr>) -> Result<Self> {
        let mut process = std::process::Command::new(path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .wrap_err("failed to sapwn the server")?;

        let (sender, sender_rcv) = sync_channel(0);

        let event_sender_clone = event_sender.clone();
        let stderr = process.stderr.take().unwrap();
        let stdin = process.stdin.take().unwrap();
        let stdout = process.stdout.take().unwrap();
        let handle1 = std::thread::spawn(|| redirect_log(event_sender_clone, stderr));
        let handle2 = std::thread::spawn(|| redirect_send(sender_rcv, stdin));
        let handle3 = std::thread::spawn(|| redirect_receive(event_sender, stdout));

        // TODO: Do something with handles
        Ok(Self {
            process: ChildGuard(process),
            sender,
            _handles: [handle1, handle2, handle3],
        })
    }
}

#[instrument(skip_all)]
fn redirect_log(sender: EventSender, child_log: std::process::ChildStderr) -> Result<()> {
    let lines = BufReader::new(child_log).lines();
    for line in lines {
        let line = line.wrap_err("failed to read log of language server")?;
        sender.send(Event::ServerLog(line)).ok();
    }
    Ok(())
}

#[instrument(skip_all)]
fn redirect_send(receiver: Receiver<Message>, stdin: std::process::ChildStdin) -> Result<()> {
    let mut writer = BufWriter::new(stdin);
    receiver
        .into_iter()
        .try_for_each(|it| it.write(&mut writer))
        .wrap_err("failed to write message to server")
}

#[instrument(skip_all)]
fn redirect_receive(sender: EventSender, stdout: std::process::ChildStdout) -> Result<()> {
    let mut reader = BufReader::new(stdout);
    while let Some(msg) =
        Message::read(&mut reader).wrap_err("failed to read message from client (stdin)")?
    {
        sender.send(Event::ServerToClient(msg)).unwrap();
    }
    Ok(())
}
