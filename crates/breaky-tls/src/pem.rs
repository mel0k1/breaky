fn b64_val(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut acc = 0u32;
    let mut bits = 0u32;
    for &c in s.as_bytes() {
        let v = match b64_val(c) {
            Some(v) => v,
            None => {
                if c == b'=' || c == b'\r' || c == b'\n' || c == b' ' || c == b'\t' {
                    continue;
                }
                return None;
            }
        };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

pub fn certificates(pem: &str) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut rest = pem;
    while let Some(start) = rest.find("-----BEGIN CERTIFICATE-----") {
        let body = &rest[start + "-----BEGIN CERTIFICATE-----".len()..];
        let Some(end) = body.find("-----END CERTIFICATE-----") else {
            break;
        };
        if let Some(der) = b64_decode(&body[..end]) {
            out.push(der);
        }
        rest = &body[end + "-----END CERTIFICATE-----".len()..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{b64_decode, certificates};

    #[test]
    fn rfc4648_vectors() {
        assert_eq!(b64_decode("").unwrap(), b"");
        assert_eq!(b64_decode("Zg==").unwrap(), b"f");
        assert_eq!(b64_decode("Zm8=").unwrap(), b"fo");
        assert_eq!(b64_decode("Zm9v").unwrap(), b"foo");
        assert_eq!(b64_decode("Zm9vYg==").unwrap(), b"foob");
        assert_eq!(b64_decode("Zm9vYmE=").unwrap(), b"fooba");
        assert_eq!(b64_decode("Zm9vYmFy").unwrap(), b"foobar");
        assert_eq!(b64_decode("TWFu").unwrap(), b"Man");
    }

    #[test]
    fn whitespace_and_newlines() {
        assert_eq!(
            b64_decode("Zm9v\nYmFy\r\n").unwrap(),
            b"foobar"
        );
        assert_eq!(b64_decode("  Zm9v Ym\tFy  ").unwrap(), b"foobar");
        assert!(b64_decode("Zm9!*").is_none());
    }

    #[test]
    fn pem_extraction() {
        let pem = "junk before\n-----BEGIN CERTIFICATE-----\nZm9v\n-----END CERTIFICATE-----\ntail\n-----BEGIN CERTIFICATE-----\nZm9vYmE=\n-----END CERTIFICATE-----\n";
        let certs = certificates(pem);
        assert_eq!(certs.len(), 2);
        assert_eq!(certs[0], b"foo");
        assert_eq!(certs[1], b"fooba");
        assert!(certificates("no certs here").is_empty());
        assert!(certificates("-----BEGIN CERTIFICATE-----\nZm9v\n").is_empty());
        assert!(certificates("-----BEGIN CERTIFICATE-----\n!!!\n-----END CERTIFICATE-----").is_empty());
    }
}
