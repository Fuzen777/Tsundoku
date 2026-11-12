//! A web server on this PC for tests, answering each request as the test says: the answers of a site Tsundoku reads,
//! without the site.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

/// A request the server received.
pub struct Request {
    pub path: String,
    /// Its headers, with their names in lowercase.
    pub headers: Vec<(String, String)>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// How the server answers a request.
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.into(),
        }
    }

    pub fn with(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }
}

/// The server, which stops answering once the test ends.
pub struct TestServer {
    /// "http://127.0.0.1:<port>".
    pub address: String,
}

impl TestServer {
    /// Starts a server answering every request with `answer`'s reply, each connection on a thread of its own.
    pub fn start(answer: impl Fn(&Request) -> Reply + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
        let address = format!("http://{}", listener.local_addr().expect("its address"));
        let answer = Arc::new(answer);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let answer = Arc::clone(&answer);
                thread::spawn(move || serve(stream, answer.as_ref()));
            }
        });
        Self { address }
    }

    /// The address of `path` on the server.
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.address)
    }
}

fn serve(stream: TcpStream, answer: &(dyn Fn(&Request) -> Reply + Send + Sync)) {
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
    let mut headers = Vec::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
        }
    }
    let reply = answer(&Request { path, headers });
    let mut head = format!(
        "HTTP/1.1 {} Answer\r\nContent-Length: {}\r\nConnection: close\r\n",
        reply.status,
        reply.body.len()
    );
    for (name, value) in &reply.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let mut stream = &stream;
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&reply.body);
    let _ = stream.flush();
}
