use std::io::{Read, Write};
use std::net::TcpStream;

fn get(host: &str, port: u16) -> String {
    let stream = TcpStream::connect((host, port)).unwrap();
    let mut client = tls::connect(stream, host).unwrap();
    client
        .write_all(
            format!(
                "GET / HTTP/1.1\r\nHost: {}\r\nUser-Agent: breaky\r\nConnection: close\r\n\r\n",
                host
            )
            .as_bytes(),
        )
        .unwrap();
    let mut buf = Vec::new();
    client.read_to_end(&mut buf).unwrap();
    client.close().unwrap();
    String::from_utf8_lossy(&buf).into_owned()
}

#[test]
#[ignore]
fn https_example_com() {
    let body = get("example.com", 443);
    let lower = body.to_lowercase();
    assert!(lower.contains("200 ok"), "status line missing: {}", body);
    assert!(lower.contains("example domain"));
}

#[test]
#[ignore]
fn local_rsa_server() {
    let body = get("127.0.0.1", 9443);
    assert!(
        body.to_lowercase().contains("200 ok"),
        "no response: {}",
        body
    );
}

#[test]
#[ignore]
fn local_ec_server() {
    let body = get("127.0.0.1", 9444);
    assert!(
        body.to_lowercase().contains("200 ok"),
        "no response: {}",
        body
    );
}

#[test]
#[ignore]
fn rejects_wrong_host() {
    let stream = TcpStream::connect(("127.0.0.1", 9443)).unwrap();
    assert!(tls::connect(stream, "other.example").is_err());
}
