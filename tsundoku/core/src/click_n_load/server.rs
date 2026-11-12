//! The receiver: a small HTTP server on this PC's own addresses only (127.0.0.1, and [::1] when it binds, for pages
//! that say "localhost"), strict by design: one request per connection, a head of at most 16 KiB, a body of at most
//! 2 MiB, 10 seconds for the whole request, and a few connections at once. Anything else is refused.

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, Shutdown, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::{ClickNLoadListener, Reply, Request, Senders, answer};

/// The longest request head read.
pub(super) const MOST_HEAD: usize = 16 << 10;
/// The longest body read.
pub(super) const MOST_BODY: usize = 2 << 20;
/// How long a request may take to arrive, and its answer to be taken.
const TIME_LIMIT: Duration = Duration::from_secs(10);
/// The most connections handled at once; others are closed at once.
const MOST_AT_ONCE: usize = 8;
/// How often the receiver looks for new connections and for being stopped.
const POLL: Duration = Duration::from_millis(50);
/// Sent as the Server header.
const SERVER: &str = concat!("Tsundoku/", env!("CARGO_PKG_VERSION"));

/// Why the receiver couldn't start.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StartError {
    /// Another app listens on the port.
    #[error("another app listens on the port")]
    PortTaken,
    #[error("{0}")]
    Failed(String),
}

/// A running receiver, which stops when dropped.
pub struct Receiver {
    port: u16,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Receiver {
    /// Listens on `port` (0 for a free one, in tests) of 127.0.0.1, and of [::1] when that binds too, taking links from
    /// `senders` for `listener`.
    pub fn start(
        port: u16,
        senders: Senders,
        listener: Arc<dyn ClickNLoadListener>,
    ) -> Result<Self, StartError> {
        let ipv4 = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).map_err(|error| {
            if error.kind() == io::ErrorKind::AddrInUse {
                StartError::PortTaken
            } else {
                StartError::Failed(error.to_string())
            }
        })?;
        let port = ipv4
            .local_addr()
            .map_err(|error| StartError::Failed(error.to_string()))?
            .port();
        // IPv6 is a help, not a need: a PC without it, or a port taken there only, still has 127.0.0.1.
        let ipv6 = TcpListener::bind((Ipv6Addr::LOCALHOST, port)).ok();
        let listeners: Vec<TcpListener> = std::iter::once(ipv4).chain(ipv6).collect();
        for listening in &listeners {
            listening
                .set_nonblocking(true)
                .map_err(|error| StartError::Failed(error.to_string()))?;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let context = Arc::new(Context {
            port,
            senders,
            listener,
            at_once: AtomicUsize::new(0),
        });
        let thread = thread::spawn(move || accept(&listeners, &stopping, &context));
        Ok(Self {
            port,
            stop,
            thread: Some(thread),
        })
    }

    /// The port it listens on.
    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for Receiver {
    /// Stops listening, freeing the port before it returns.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Context {
    port: u16,
    senders: Senders,
    listener: Arc<dyn ClickNLoadListener>,
    at_once: AtomicUsize,
}

/// Takes connections until stopped, each on a thread of its own.
fn accept(listeners: &[TcpListener], stop: &AtomicBool, context: &Arc<Context>) {
    while !stop.load(Ordering::Relaxed) {
        let mut took = false;
        for listening in listeners {
            if let Ok((stream, _)) = listening.accept() {
                took = true;
                if context.at_once.fetch_add(1, Ordering::SeqCst) >= MOST_AT_ONCE {
                    context.at_once.fetch_sub(1, Ordering::SeqCst);
                    let _ = stream.shutdown(Shutdown::Both);
                    continue;
                }
                let context = Arc::clone(context);
                thread::spawn(move || {
                    serve(stream, &context);
                    context.at_once.fetch_sub(1, Ordering::SeqCst);
                });
            }
        }
        if !took {
            thread::sleep(POLL);
        }
    }
}

/// Reads one request and answers it.
fn serve(mut stream: TcpStream, context: &Context) {
    // An accepted connection may take after its listener, which doesn't wait.
    if stream.set_nonblocking(false).is_err()
        || stream.set_read_timeout(Some(TIME_LIMIT)).is_err()
        || stream.set_write_timeout(Some(TIME_LIMIT)).is_err()
    {
        return;
    }
    let reply = match read_request(&mut stream, Instant::now() + TIME_LIMIT) {
        Ok(request) => answer(
            &request,
            context.port,
            &context.senders,
            context.listener.as_ref(),
        ),
        Err(Some(reply)) => reply,
        Err(None) => return,
    };
    let _ = stream.write_all(&written(&reply));
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}

/// The request on `stream`, or the answer for one that can't be taken (`None` when nothing can be answered).
pub(super) fn read_request(
    stream: &mut impl Read,
    deadline: Instant,
) -> Result<Request, Option<Reply>> {
    let mut received = Vec::new();
    let mut chunk = [0; 4096];
    let head_end = loop {
        if let Some(end) = find(&received, b"\r\n\r\n") {
            break end;
        }
        if received.len() > MOST_HEAD {
            return Err(Some(Reply::text(431, "The request's head is too long.")));
        }
        if Instant::now() > deadline {
            return Err(Some(Reply::text(408, "The request took too long.")));
        }
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return Err(None),
            Ok(count) => received.extend_from_slice(&chunk[..count]),
        }
    };
    if head_end > MOST_HEAD {
        return Err(Some(Reply::text(431, "The request's head is too long.")));
    }
    let head = String::from_utf8_lossy(&received[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split(' ');
    let (Some(method), Some(target), Some(version)) = (first.next(), first.next(), first.next())
    else {
        return Err(Some(Reply::text(400, "That isn't an HTTP request.")));
    };
    if !version.starts_with("HTTP/1.") {
        return Err(Some(Reply::text(400, "That isn't an HTTP/1 request.")));
    }
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    let header = |name: &str| {
        headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };
    if header("transfer-encoding").is_some() {
        return Err(Some(Reply::text(
            411,
            "Send the request with a Content-Length.",
        )));
    }
    let length = match header("content-length") {
        None => 0,
        Some(value) => value.parse::<usize>().map_err(|_| {
            Some(Reply::text(
                400,
                "The request's Content-Length isn't a number.",
            ))
        })?,
    };
    if length > MOST_BODY {
        return Err(Some(Reply::text(413, "The request is too long.")));
    }
    let mut body = received.split_off(head_end + 4);
    while body.len() < length {
        if Instant::now() > deadline {
            return Err(Some(Reply::text(408, "The request took too long.")));
        }
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return Err(None),
            Ok(count) => body.extend_from_slice(&chunk[..count]),
        }
    }
    body.truncate(length);
    let path = target
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .to_owned();
    Ok(Request {
        method: method.to_owned(),
        path,
        headers,
        body,
    })
}

/// Where `needle` first starts in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// The answer as sent, closing the connection after it.
pub(super) fn written(reply: &Reply) -> Vec<u8> {
    let reason = match reply.status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        411 => "Length Required",
        413 => "Content Too Large",
        431 => "Request Header Fields Too Large",
        _ => "Internal Server Error",
    };
    let mut head = format!(
        "HTTP/1.1 {} {reason}\r\nServer: {SERVER}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n",
        reply.status,
        reply.content_type,
        reply.body.len()
    );
    for (name, value) in &reply.headers {
        // A value with a line break would start a header of its own.
        if !value.contains(['\r', '\n']) {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
    }
    head.push_str("\r\n");
    let mut bytes = head.into_bytes();
    bytes.extend_from_slice(reply.body.as_bytes());
    bytes
}
