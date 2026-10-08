//! Captures tracing output so the log window can show it.

use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use std::io::{self, Write};
use std::sync::OnceLock;
use tracing::{Level, Metadata};
use tracing_subscriber::filter::Targets;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::fmt::time::ChronoLocal;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Clone, Debug, PartialEq)]
pub struct LogLine {
    pub level: Level,
    pub text: String,
}

static SINK: OnceLock<UnboundedSender<LogLine>> = OnceLock::new();

/// Sets up tracing and returns the stream of formatted log lines.
pub fn init() -> UnboundedReceiver<LogLine> {
    let (tx, rx) = unbounded();
    SINK.set(tx).expect("logging initialized twice");

    let filter = Targets::new()
        .with_default(Level::WARN)
        .with_target("flixparty_core", Level::DEBUG)
        .with_target("flixparty", Level::DEBUG);

    let capture = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_timer(ChronoLocal::new("%H:%M:%S%.3f".into()))
        .with_writer(CaptureWriter);

    let stdout = tracing_subscriber::fmt::layer().with_writer(io::stdout);

    tracing_subscriber::registry()
        .with(filter)
        .with(capture)
        .with(stdout)
        .init();

    rx
}

struct CaptureWriter;

impl<'a> MakeWriter<'a> for CaptureWriter {
    type Writer = LineWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LineWriter::new(Level::INFO)
    }

    fn make_writer_for(&'a self, meta: &Metadata<'_>) -> Self::Writer {
        LineWriter::new(*meta.level())
    }
}

/// Buffers one formatted event and sends it when dropped.
struct LineWriter {
    level: Level,
    buf: Vec<u8>,
}

impl LineWriter {
    fn new(level: Level) -> Self {
        Self {
            level,
            buf: Vec::new(),
        }
    }
}

impl Write for LineWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for LineWriter {
    fn drop(&mut self) {
        let text = String::from_utf8_lossy(&self.buf).trim_end().to_string();
        if text.is_empty() {
            return;
        }
        if let Some(sink) = SINK.get() {
            let _ = sink.unbounded_send(LogLine {
                level: self.level,
                text,
            });
        }
    }
}
