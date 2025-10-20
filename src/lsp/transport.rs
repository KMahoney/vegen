use serde::Serialize;
use serde_json::{json, Value};
use std::fmt;
use std::io::{self, BufRead, BufWriter, Write};

/// Represents an error that can occur while reading an LSP message from stdin.
#[derive(Debug)]
pub enum ReadError {
    Eof,
    Io(io::Error),
    Utf8(std::string::FromUtf8Error),
    MissingContentLength,
    InvalidHeader(String),
    Json(serde_json::Error),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::Eof => write!(f, "unexpected end of input"),
            ReadError::Io(err) => write!(f, "io error: {}", err),
            ReadError::Utf8(err) => write!(f, "utf8 decoding error: {}", err),
            ReadError::MissingContentLength => write!(f, "missing Content-Length header"),
            ReadError::InvalidHeader(header) => write!(f, "invalid header: {}", header),
            ReadError::Json(err) => write!(f, "failed to decode JSON payload: {}", err),
        }
    }
}

impl std::error::Error for ReadError {}

/// Reads a single Language Server Protocol message from the provided reader.
pub fn read_message<R: BufRead>(reader: &mut R) -> Result<Value, ReadError> {
    let mut content_length: Option<usize> = None;
    let mut header = String::new();

    loop {
        header.clear();
        let bytes_read = reader.read_line(&mut header).map_err(ReadError::Io)?;

        if bytes_read == 0 {
            return Err(ReadError::Eof);
        }

        let trimmed = header.trim_end_matches(['\r', '\n']);

        if trimmed.is_empty() {
            break;
        }

        let mut parts = trimmed.splitn(2, ':');
        let name = parts
            .next()
            .map(str::trim)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let value = parts.next().map(str::trim);

        match (name.as_str(), value) {
            ("content-length", Some(v)) => {
                let len = v
                    .parse::<usize>()
                    .map_err(|_| ReadError::InvalidHeader(trimmed.to_string()))?;
                content_length = Some(len);
            }
            (_, _) => {
                // Ignore other headers for now
            }
        }
    }

    let content_length = content_length.ok_or(ReadError::MissingContentLength)?;
    let mut buffer = vec![0u8; content_length];
    reader.read_exact(&mut buffer).map_err(ReadError::Io)?;

    let payload = String::from_utf8(buffer).map_err(ReadError::Utf8)?;
    let value = serde_json::from_str::<Value>(&payload).map_err(ReadError::Json)?;
    Ok(value)
}

/// Convenience wrapper around a buffered writer for emitting LSP responses.
pub struct Sender<W: Write> {
    writer: BufWriter<W>,
}

impl<W: Write> Sender<W> {
    pub fn new(inner: W) -> Self {
        Self {
            writer: BufWriter::new(inner),
        }
    }

    pub fn send_value(&mut self, value: Value) -> io::Result<()> {
        let payload = serde_json::to_string(&value).expect("failed to encode JSON value");
        self.send_raw(&payload)
    }

    pub fn send_response<R: Serialize>(&mut self, id: Value, result: R) -> io::Result<()> {
        let result_value = serde_json::to_value(result).expect("failed to serialize response");
        let envelope = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result_value,
        });
        self.send_value(envelope)
    }

    pub fn send_error(
        &mut self,
        id: Value,
        code: i32,
        message: impl Into<String>,
    ) -> io::Result<()> {
        let envelope = json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": code,
                "message": message.into(),
            }
        });
        self.send_value(envelope)
    }

    pub fn send_notification<P: Serialize>(&mut self, method: &str, params: P) -> io::Result<()> {
        let params_value = serde_json::to_value(params).expect("failed to serialize params");
        let envelope = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params_value,
        });
        self.send_value(envelope)
    }

    fn send_raw(&mut self, payload: &str) -> io::Result<()> {
        write!(self.writer, "Content-Length: {}\r\n\r\n", payload.len())?;
        self.writer.write_all(payload.as_bytes())?;
        self.writer.flush()
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl ReadError {
    pub fn is_eof(&self) -> bool {
        matches!(self, ReadError::Eof)
    }
}
