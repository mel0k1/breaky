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
fn https_google_com() {
    let body = get("google.com", 443);
    let lower = body.to_lowercase();
    assert!(
        lower.contains("301 moved") || lower.contains("302 found") || lower.contains("200 ok"),
        "unexpected status: {}",
        body
    );
    assert!(lower.contains("location:"), "no redirect target: {}", body);
}

#[test]
#[ignore]
fn https_pypi_org() {
    let body = get("pypi.org", 443);
    let lower = body.to_lowercase();
    assert!(lower.contains("200 ok"), "unexpected status: {}", body);
    assert!(lower.contains("<!doctype html>") || lower.contains("<html"), "{}", body);
}
