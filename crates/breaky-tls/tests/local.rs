use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use tls::{pem, x509};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

fn spawn_server(port: u16, suite: &str) -> Child {
    Command::new("openssl")
        .args([
            "s_server",
            "-accept",
            &format!("127.0.0.1:{}", port),
            "-tls1_3",
            "-ciphersuites",
            suite,
            "-cert",
            &format!("{DIR}/tls13-server.pem"),
            "-key",
            &format!("{DIR}/tls13-server.key"),
            "-cert_chain",
            &format!("{DIR}/tls13-root.pem"),
            "-www",
            "-naccept",
            "1",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("openssl s_server")
}

fn roots() -> Vec<x509::Cert> {
    let text = std::fs::read_to_string(format!("{DIR}/tls13-root.pem")).expect("fixture missing");
    pem::certificates(&text)
        .iter()
        .map(|d| x509::parse(d).expect("root"))
        .collect()
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn fetch(port: u16, suite: &str) -> String {
    let mut child = spawn_server(port, suite);
    let mut stream = None;
    for _ in 0..30 {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(s) => {
                stream = Some(s);
                break;
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    let stream = stream.expect("s_server did not start");
    let mut client = tls::connect_with_roots(stream, "localhost", &roots()).unwrap();
    client
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut buf = Vec::new();
    client.read_to_end(&mut buf).unwrap();
    client.close().unwrap();
    let _ = child.kill();
    let _ = child.wait();
    String::from_utf8_lossy(&buf).into_owned()
}

#[test]
fn local_tls13_suites() {
    let body = fetch(free_port(), "TLS_AES_128_GCM_SHA256");
    assert!(body.contains("200 ok") || body.contains("s_server"), "{}", body);
    let body = fetch(free_port(), "TLS_AES_256_GCM_SHA384");
    assert!(body.contains("200 ok") || body.contains("s_server"), "{}", body);
}
