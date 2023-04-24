use bytes::{Buf as _, BufMut as _, BytesMut};
use lsp_server::Message;
use thiserror::Error;
use tokio_util::codec::{Decoder, Encoder};

#[derive(Error, Debug)]
pub enum DecodeError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error("Malformed header ({_0}): `{_1:?}`")]
    MalformedHeader(&'static str, Vec<u8>),
    #[error("Missing Content-Length header")]
    MissingContentLength,
}

pub struct MessageCodec;

impl Encoder<Message> for MessageCodec {
    type Error = std::io::Error;

    fn encode(&mut self, item: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        item.write(&mut dst.writer())
    }
}

impl Decoder for MessageCodec {
    type Item = Message;

    type Error = DecodeError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut slice = src.as_ref();
        let mut len = None;
        loop {
            let Some(line_end) = slice.iter().position(|b| *b == b'\n') else {
                return Ok(None)
            };
            let (line, new_slice) = slice.split_at(line_end + 1);
            slice = new_slice;
            let line = line
                .strip_suffix(b"\r\n")
                .ok_or_else(|| DecodeError::MalformedHeader("no CRLF", line.to_vec()))?;
            if line.is_empty() {
                break;
            }
            let colon = line
                .iter()
                .position(|b| *b == b':')
                .ok_or_else(|| DecodeError::MalformedHeader("no colon", line.to_vec()))?;
            let (head, value) = line.split_at(colon + 1);
            if head != b"Content-Length:".as_slice() {
                continue;
            }
            let value = std::str::from_utf8(value)
                .map_err(|_| DecodeError::MalformedHeader("invalid as utf-8", line.to_vec()))?
                .trim();
            len = Some(value.parse::<usize>().map_err(|_| {
                DecodeError::MalformedHeader(
                    "unable to parse Content-Length as an usize",
                    line.to_vec(),
                )
            })?);
        }
        let content_len = len.ok_or(DecodeError::MissingContentLength)?;
        if slice.len() < content_len {
            src.reserve(content_len - slice.len());
            return Ok(None);
        }
        let mut reader = std::mem::take(src).reader();
        let ret = Message::read(&mut reader);
        *src = reader.into_inner();
        Ok(Some(ret?.expect(
            "We checked that it's not the end of the input...",
        )))
    }
}
