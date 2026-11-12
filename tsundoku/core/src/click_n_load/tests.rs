use std::io::Read;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use super::server::{MOST_BODY, MOST_HEAD, read_request, written};
use super::*;

/// Made with .NET's AES (see `decode`'s tests): "https://www.youtube.com/watch?v=jNQXAC9IVRw" and
/// "https://archive.org/details/test-item".
const CRYPTED: &str = "Id4uAcAc/pk1oIGvGjZpAmjMkfZbMK0ma1yOkUFm0scHF39DcL4UWZcJFOnjbeeMCPFHcaGTMSd3SfDtIuX59RmiBWSZpu/a7qswYDJfJPpKrRpvqooZaRy63fV10+2R";
const JK: &str = "function f(){ return '31323334353637383930616263646566';}";
const PORT_HERE: u16 = 9666;

/// Keeps what the receiver hands on.
#[derive(Default)]
struct Kept {
    received: Mutex<Vec<(String, Vec<String>)>>,
    refused: Mutex<Vec<Option<String>>>,
}

impl ClickNLoadListener for Kept {
    fn received(&self, site: &str, links: &[String]) -> bool {
        self.received
            .lock()
            .unwrap()
            .push((site.to_owned(), links.to_vec()));
        true
    }

    fn refused(&self, host: Option<&str>) {
        self.refused.lock().unwrap().push(host.map(str::to_owned));
    }
}

impl Kept {
    fn received(&self) -> Vec<(String, Vec<String>)> {
        self.received.lock().unwrap().clone()
    }

    fn refused(&self) -> Vec<Option<String>> {
        self.refused.lock().unwrap().clone()
    }
}

fn request(method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> Request {
    let mut all: Vec<(String, String)> =
        vec![("host".to_owned(), format!("127.0.0.1:{PORT_HERE}"))];
    for (name, value) in headers {
        all.retain(|(key, _)| key != name);
        all.push(((*name).to_owned(), (*value).to_owned()));
    }
    Request {
        method: method.to_owned(),
        path: path.to_owned(),
        headers: all,
        body: body.as_bytes().to_vec(),
    }
}

fn answered(request: &Request, kept: &Kept) -> Reply {
    answer(request, PORT_HERE, &Senders::listed(), kept)
}

/// A form body, encoded as a browser encodes one.
fn form(fields: &[(&str, &str)]) -> String {
    crate::hosts::form(fields)
}

fn header<'a>(reply: &'a Reply, name: &str) -> Option<&'a str> {
    reply
        .headers
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value.as_str())
}

const FILECRYPT: &str = "https://filecrypt.cc";

fn expected_links() -> Vec<String> {
    vec![
        "https://www.youtube.com/watch?v=jNQXAC9IVRw".to_owned(),
        "https://archive.org/details/test-item".to_owned(),
    ]
}

#[test]
fn answers_only_requests_to_this_pcs_own_address() {
    let kept = Kept::default();
    for host in [
        "127.0.0.1:9666",
        "localhost:9666",
        "[::1]:9666",
        "LOCALHOST:9666",
    ] {
        let reply = answered(&request("GET", "/jdcheck.js", &[("host", host)], ""), &kept);
        assert_eq!(reply.status, 200, "{host}");
    }
    for host in [
        "filecrypt.cc",
        "evil.example:9666",
        "127.0.0.1:9667",
        "127.0.0.1",
        "",
    ] {
        let reply = answered(&request("GET", "/jdcheck.js", &[("host", host)], ""), &kept);
        assert_eq!(reply.status, 403, "{host}");
    }
}

#[test]
fn answers_the_probes_as_jdownloader_does() {
    let kept = Kept::default();
    let reply = answered(&request("GET", "/jdcheck.js", &[], ""), &kept);
    assert_eq!(reply.status, 200);
    assert_eq!(reply.content_type, "text/javascript; charset=utf-8");
    assert!(reply.body.starts_with("jdownloader=true;\r\nvar version='"));
    for path in ["/flash", "/flash/"] {
        let reply = answered(&request("GET", path, &[], ""), &kept);
        assert_eq!(
            (reply.status, reply.body.as_str()),
            (200, "JDownloader"),
            "{path}"
        );
    }

    // A listed page gets its answer with CORS; another page is refused and the app hears of it.
    let reply = answered(
        &request(
            "GET",
            "/jdcheck.js",
            &[("origin", "https://www.filecrypt.cc")],
            "",
        ),
        &kept,
    );
    assert_eq!(
        header(&reply, "Access-Control-Allow-Origin"),
        Some("https://www.filecrypt.cc")
    );
    assert_eq!(header(&reply, "Vary"), Some("Origin"));
    let reply = answered(
        &request(
            "GET",
            "/jdcheck.js",
            &[("referer", "https://filecrypt.co/Container/ABC.html")],
            "",
        ),
        &kept,
    );
    assert_eq!(reply.status, 200);
    let reply = answered(
        &request(
            "GET",
            "/flash/",
            &[("referer", "https://evil.example/page")],
            "",
        ),
        &kept,
    );
    assert_eq!(reply.status, 403);
    assert_eq!(kept.refused(), [Some("evil.example".to_owned())]);
    assert!(kept.received().is_empty());
}

#[test]
fn takes_a_listed_sites_links() {
    let kept = Kept::default();
    let body = form(&[
        ("passwords", ""),
        ("source", "https://filecrypt.cc/Container/ABC.html"),
        ("jk", JK),
        ("crypted", CRYPTED),
    ]);
    let reply = answered(
        &request(
            "POST",
            "/flash/addcrypted2",
            &[
                ("origin", FILECRYPT),
                ("content-type", "application/x-www-form-urlencoded"),
            ],
            &body,
        ),
        &kept,
    );
    assert_eq!((reply.status, reply.body.as_str()), (200, "success\r\n"));
    assert_eq!(
        header(&reply, "Access-Control-Allow-Origin"),
        Some(FILECRYPT)
    );
    assert_eq!(
        kept.received(),
        [("FileCrypt".to_owned(), expected_links())]
    );

    // A "+" sent unencoded arrives as a space, which stands for it again.
    let raw = format!(
        "jk={}&crypted={CRYPTED}",
        form(&[("x", JK)]).trim_start_matches("x=")
    );
    assert!(raw.contains('+'));
    let reply = answered(
        &request(
            "POST",
            "/flash/addcrypted2/",
            &[("origin", FILECRYPT)],
            &raw,
        ),
        &kept,
    );
    assert_eq!(reply.status, 200);
    assert_eq!(kept.received()[1].1, expected_links());

    // Plain links.
    let reply = answered(
        &request(
            "POST",
            "/flash/add",
            &[("referer", "https://filecrypt.to/Container/ABC.html")],
            &form(&[(
                "urls",
                "https://example.com/a\r\nhttps://example.com/b\r\nnot a link",
            )]),
        ),
        &kept,
    );
    assert_eq!(reply.status, 200);
    assert_eq!(
        kept.received()[2],
        (
            "FileCrypt".to_owned(),
            vec![
                "https://example.com/a".to_owned(),
                "https://example.com/b".to_owned()
            ]
        )
    );
}

#[test]
fn knows_the_sender_by_origin_then_referer_then_source() {
    let kept = Kept::default();
    let post = |headers: &[(&str, &str)], source: &str| {
        let body = form(&[("source", source), ("jk", JK), ("crypted", CRYPTED)]);
        answered(
            &request("POST", "/flash/addcrypted2", headers, &body),
            &kept,
        )
        .status
    };
    let listed_source = "https://filecrypt.cc/Container/ABC.html";
    assert_eq!(post(&[("origin", FILECRYPT)], ""), 200, "Origin");
    assert_eq!(
        post(&[("origin", "null")], listed_source),
        200,
        "a null Origin, then source"
    );
    assert_eq!(
        post(
            &[("referer", "https://www.filecrypt.co/Container/ABC.html")],
            ""
        ),
        200,
        "Referer alone"
    );
    assert_eq!(post(&[], listed_source), 200, "source alone");

    assert_eq!(
        post(&[("origin", "https://evil.example")], listed_source),
        403,
        "an unlisted Origin"
    );
    assert_eq!(
        post(
            &[("origin", "null"), ("referer", "https://evil.example/")],
            listed_source
        ),
        403,
        "an unlisted Referer"
    );
    assert_eq!(post(&[], ""), 403, "nothing at all");
    assert_eq!(
        post(&[], "https://evil.example/x"),
        403,
        "an unlisted source"
    );
    assert_eq!(
        post(&[("origin", "https://filecrypt.cc.evil.example")], ""),
        403
    );
    assert_eq!(kept.received().len(), 4);
    assert_eq!(
        kept.refused(),
        [
            Some("evil.example".to_owned()),
            Some("evil.example".to_owned()),
            None,
            Some("evil.example".to_owned()),
            Some("filecrypt.cc.evil.example".to_owned()),
        ]
    );
    let reply = answered(&request("POST", "/flash/add", &[], ""), &kept);
    assert_eq!(
        reply.body,
        "Tsundoku only takes Click'n'Load links from FileCrypt."
    );
}

#[test]
fn a_test_page_counts_only_when_given() {
    let kept = Kept::default();
    let body = form(&[("jk", JK), ("crypted", CRYPTED)]);
    let post = request(
        "POST",
        "/flash/addcrypted2",
        &[("origin", "http://127.0.0.1:8123")],
        &body,
    );
    assert_eq!(answered(&post, &kept).status, 403);
    let reply = answer(
        &post,
        PORT_HERE,
        &Senders::with_test_host("127.0.0.1"),
        &kept,
    );
    assert_eq!(reply.status, 200);
    assert_eq!(kept.received()[0].0, "Test Page");
}

#[test]
fn says_why_a_post_gave_nothing() {
    let kept = Kept::default();
    let post = |path: &str, body: &str, headers: &[(&str, &str)]| {
        let mut all = vec![("origin", FILECRYPT)];
        all.extend_from_slice(headers);
        answered(&request("POST", path, &all, body), &kept)
    };
    let reply = post("/flash/addcrypted", &form(&[("crypted", "AAAA")]), &[]);
    assert_eq!(reply.status, 400);
    assert!(reply.body.contains("DLC"), "{}", reply.body);
    let reply = post(
        "/flash/addcrypted2",
        &form(&[("jk", "function f(){ return k; }"), ("crypted", CRYPTED)]),
        &[],
    );
    assert_eq!(
        (reply.status, reply.body),
        (400, DecodeError::KeyUnreadable.to_string())
    );
    let reply = post(
        "/flash/addcrypted2",
        &form(&[("jk", JK), ("crypted", "!!!")]),
        &[],
    );
    assert_eq!(
        (reply.status, reply.body),
        (400, DecodeError::BadBase64.to_string())
    );
    let reply = post("/flash/add", &form(&[("urls", "nothing here")]), &[]);
    assert_eq!(
        (reply.status, reply.body),
        (400, DecodeError::NoLinks.to_string())
    );
    let reply = post(
        "/flash/addcrypted2",
        "--x\r\nContent-Disposition: form-data; name=\"jk\"\r\n\r\nx\r\n--x--",
        &[("content-type", "multipart/form-data; boundary=x")],
    );
    assert_eq!(reply.status, 400);
    assert!(kept.received().is_empty());
}

#[test]
fn answers_a_listed_pages_preflight() {
    let kept = Kept::default();
    let reply = answered(
        &request(
            "OPTIONS",
            "/flash/addcrypted2",
            &[
                ("origin", FILECRYPT),
                ("access-control-request-method", "POST"),
                ("access-control-request-headers", "content-type"),
                ("access-control-request-private-network", "true"),
            ],
            "",
        ),
        &kept,
    );
    assert_eq!(reply.status, 204);
    assert_eq!(
        header(&reply, "Access-Control-Allow-Origin"),
        Some(FILECRYPT)
    );
    assert_eq!(
        header(&reply, "Access-Control-Allow-Methods"),
        Some("GET, POST, OPTIONS")
    );
    assert_eq!(
        header(&reply, "Access-Control-Allow-Headers"),
        Some("content-type")
    );
    assert_eq!(
        header(&reply, "Access-Control-Allow-Private-Network"),
        Some("true")
    );

    let reply = answered(
        &request(
            "OPTIONS",
            "/flash/addcrypted2",
            &[("origin", "https://evil.example")],
            "",
        ),
        &kept,
    );
    assert_eq!(reply.status, 403);
    assert_eq!(header(&reply, "Access-Control-Allow-Origin"), None);
}

#[test]
fn other_routes_and_methods_are_refused() {
    let kept = Kept::default();
    assert_eq!(answered(&request("GET", "/", &[], ""), &kept).status, 404);
    assert_eq!(
        answered(&request("GET", "/flash/addcrypted2", &[], ""), &kept).status,
        405
    );
    assert_eq!(
        answered(&request("DELETE", "/jdcheck.js", &[], ""), &kept).status,
        405
    );
    assert_eq!(
        answered(&request("POST", "/crossdomain.xml", &[], ""), &kept).status,
        404
    );
}

#[test]
fn reads_requests_within_their_limits() {
    let read = |bytes: Vec<u8>| {
        read_request(
            &mut bytes.as_slice(),
            std::time::Instant::now() + std::time::Duration::from_secs(5),
        )
    };
    let request = read(
        b"POST /flash/add?x=1 HTTP/1.1\r\nHost: 127.0.0.1:9666\r\nContent-Length: 5\r\nORIGIN: https://filecrypt.cc\r\n\r\nurls=extra"
            .to_vec(),
    )
    .unwrap();
    assert_eq!(
        (request.method.as_str(), request.path.as_str()),
        ("POST", "/flash/add")
    );
    assert_eq!(request.body, b"urls=");
    assert_eq!(request.header("origin"), Some("https://filecrypt.cc"));

    let long_head = format!("GET / HTTP/1.1\r\nX: {}\r\n\r\n", "a".repeat(MOST_HEAD));
    assert_eq!(
        read(long_head.into_bytes())
            .unwrap_err()
            .map(|reply| reply.status),
        Some(431)
    );
    let too_long = format!(
        "POST /flash/add HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
        MOST_BODY + 1
    );
    assert_eq!(
        read(too_long.into_bytes())
            .unwrap_err()
            .map(|reply| reply.status),
        Some(413)
    );
    let chunked = b"POST /flash/add HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec();
    assert_eq!(
        read(chunked).unwrap_err().map(|reply| reply.status),
        Some(411)
    );
    assert_eq!(
        read(b"HELLO\r\n\r\n".to_vec())
            .unwrap_err()
            .map(|reply| reply.status),
        Some(400)
    );
    // A connection that closes early gets no answer.
    assert!(
        read(b"POST / HTTP/1.1\r\nContent-Length: 9\r\n\r\nshort".to_vec())
            .unwrap_err()
            .is_none()
    );

    let sent = String::from_utf8(written(
        &Reply::text(200, "success\r\n").with("X-Bad", "a\r\nInjected: yes"),
    ))
    .unwrap();
    assert!(sent.starts_with("HTTP/1.1 200 OK\r\nServer: Tsundoku/"));
    assert!(sent.contains("Content-Length: 9\r\n"));
    assert!(
        !sent.contains("Injected"),
        "a header with a line break isn't written"
    );
}

#[test]
fn a_real_page_posting_a_form_reaches_the_listener() {
    let kept = Arc::new(Kept::default());
    let listener: Arc<dyn ClickNLoadListener> = Arc::clone(&kept) as Arc<dyn ClickNLoadListener>;
    let receiver = Receiver::start(0, Senders::listed(), listener).unwrap();
    let address = format!("http://127.0.0.1:{}", receiver.port());
    let client = reqwest::blocking::Client::new();

    let probe = client.get(format!("{address}/jdcheck.js")).send().unwrap();
    assert_eq!(probe.status(), 200);
    assert!(
        probe.headers()["server"]
            .to_str()
            .unwrap()
            .starts_with("Tsundoku/")
    );
    assert!(probe.text().unwrap().starts_with("jdownloader=true;"));

    let reply = client
        .post(format!("{address}/flash/addcrypted2"))
        .header("Origin", FILECRYPT)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form(&[
            ("jk", JK),
            ("crypted", CRYPTED),
            ("source", ""),
            ("passwords", ""),
        ]))
        .send()
        .unwrap();
    assert_eq!(reply.status(), 200);
    assert_eq!(reply.text().unwrap(), "success\r\n");
    assert_eq!(
        kept.received(),
        [("FileCrypt".to_owned(), expected_links())]
    );

    let refused = client
        .post(format!("{address}/flash/addcrypted2"))
        .header("Origin", "https://evil.example")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form(&[("jk", JK), ("crypted", CRYPTED)]))
        .send()
        .unwrap();
    assert_eq!(refused.status(), 403);

    // Another address of this PC's isn't this PC's own name.
    let mut raw = std::net::TcpStream::connect(("127.0.0.1", receiver.port())).unwrap();
    std::io::Write::write_all(
        &mut raw,
        b"GET /jdcheck.js HTTP/1.1\r\nHost: filecrypt.cc\r\n\r\n",
    )
    .unwrap();
    let mut answer = String::new();
    raw.read_to_string(&mut answer).unwrap();
    assert!(answer.starts_with("HTTP/1.1 403"), "{answer}");
}

#[test]
fn a_taken_port_says_so_and_a_stopped_receiver_frees_its_port() {
    let other = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = other.local_addr().unwrap().port();
    let kept: Arc<dyn ClickNLoadListener> = Arc::new(Kept::default());
    assert_eq!(
        Receiver::start(port, Senders::listed(), Arc::clone(&kept)).err(),
        Some(StartError::PortTaken)
    );
    drop(other);
    let receiver = Receiver::start(port, Senders::listed(), Arc::clone(&kept)).unwrap();
    drop(receiver);
    TcpListener::bind(("127.0.0.1", port)).expect("the port is free again");
}
