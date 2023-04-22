use std::{ffi::OsStr, io::Cursor};

use eyre::{Context as _, Result};
use lsp_server::Message;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader, BufWriter},
    process::{Child, ChildStderr, ChildStdin, ChildStdout},
    spawn,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};
use tracing::instrument;

use crate::event::{Event, EventSender};

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
        sender.send(Event::ServerLog(line)).ok();
    }
    Ok(())
}

#[instrument(skip_all)]
async fn redirect_send(mut receiver: UnboundedReceiver<Message>, stdin: ChildStdin) -> Result<()> {
    let mut writer = BufWriter::new(stdin);
    while let Some(msg) = receiver.recv().await {
        let mut buf = Vec::new();
        msg.write(&mut buf)
            .wrap_err("failed to serialize message")?;
        writer
            .write_all(&buf)
            .await
            .wrap_err("failed to write message to server")?;
        writer
            .flush()
            .await
            .wrap_err("failed to write message to server")?;
    }
    Ok(())
}

#[instrument(skip_all)]
async fn redirect_receive(sender: EventSender, stdout: ChildStdout) -> Result<()> {
    let mut reader = SyncRead::new(stdout);
    while let Some(msg) =
        Message::read(&mut reader).wrap_err("failed to read message from server")?
    {
        sender.send(Event::ServerToClient(msg)).unwrap();
    }
    Ok(())
}

struct SyncRead {
    current: Cursor<Vec<u8>>,
    receiver: std::sync::mpsc::Receiver<std::io::Result<Cursor<Vec<u8>>>>,
    _handle: JoinHandle<Result<()>>,
}
impl SyncRead {
    fn new(mut inner: impl 'static + Send + Unpin + AsyncRead) -> Self {
        let (sender, receiver) = std::sync::mpsc::sync_channel(5);
        let handle = spawn(async move {
            use tokio::io::AsyncReadExt;
            loop {
                let mut buf = vec![0; 2048];
                let v = inner.read(&mut buf).await.map(|n| {
                    buf.truncate(n);
                    Cursor::new(buf)
                });
                sender.send(v)?;
            }
        });
        Self {
            current: Cursor::new(vec![]),
            receiver,
            _handle: handle,
        }
    }

    fn buffer_is_empty(&self) -> bool {
        // TODO: assert!(self.current.is_empty()); when it's stabilized
        self.current.get_ref().len() as u64 <= self.current.position()
    }
    fn renew_buffer(&mut self) -> std::io::Result<bool> {
        assert!(self.buffer_is_empty());
        match self.receiver.recv() {
            Ok(Ok(cursor)) => {
                self.current = cursor;
                Ok(true)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Ok(false),
        }
    }
}

impl std::io::Read for SyncRead {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            let n = self
                .current
                .read(buf)
                .expect("read from cursor shouldn't fail");
            if n != 0 {
                return Ok(n);
            }
            if !self.renew_buffer()? {
                return Ok(0);
            }
        }
    }
}

impl std::io::BufRead for SyncRead {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        while self.buffer_is_empty() {
            if !self.renew_buffer()? {
                return Ok(&[]);
            }
        }
        std::io::BufRead::fill_buf(&mut self.current)
    }

    fn consume(&mut self, amt: usize) {
        std::io::BufRead::consume(&mut self.current, amt);
    }
}
