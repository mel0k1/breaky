pub struct Reader<'a> {
    buf: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Reader<'a> {
        Reader { buf }
    }

    pub fn tlv(&mut self) -> Option<(u8, &'a [u8])> {
        if self.buf.len() < 2 {
            return None;
        }
        let tag = self.buf[0];
        let first = self.buf[1];
        let (len, hdr) = if first < 0x80 {
            (first as usize, 2)
        } else {
            let n = (first & 0x7f) as usize;
            if n == 0 || n > 4 || self.buf.len() < 2 + n {
                return None;
            }
            let mut l = 0usize;
            for k in 0..n {
                l = (l << 8) | self.buf[2 + k] as usize;
            }
            (l, 2 + n)
        };
        if self.buf.len() < hdr + len {
            return None;
        }
        let v = &self.buf[hdr..hdr + len];
        self.buf = &self.buf[hdr + len..];
        Some((tag, v))
    }

    pub fn next(&mut self, tag: u8) -> Option<&'a [u8]> {
        let (t, v) = self.tlv()?;
        if t != tag {
            return None;
        }
        Some(v)
    }

    pub fn rest(&self) -> &'a [u8] {
        self.buf
    }
}

pub fn int_bytes(v: &[u8]) -> &[u8] {
    let mut i = 0;
    while i < v.len() && v[i] == 0 {
        i += 1;
    }
    if i == v.len() {
        &v[v.len() - 1..]
    } else {
        &v[i..]
    }
}

pub const OID_RSA: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01];
pub const OID_EC: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01];
pub const OID_P256: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];
pub const OID_SAN: &[u8] = &[0x55, 0x1d, 0x11];

#[cfg(test)]
mod tests {
    use super::{int_bytes, Reader, OID_RSA};

    #[test]
    fn tlv_walk() {
        let d = [0x30, 0x05, 0x02, 0x03, 0x01, 0x00, 0x01];
        let mut r = Reader::new(&d);
        let seq = r.next(0x30).unwrap();
        let mut sr = Reader::new(seq);
        let i = sr.next(0x02).unwrap();
        assert_eq!(i, &[0x01, 0x00, 0x01]);
        assert!(sr.next(0x02).is_none());
        let mut r = Reader::new(&d);
        assert!(r.next(0x02).is_none());
    }

    #[test]
    fn long_form_lengths() {
        let mut d = vec![0x04, 0x82, 0x01, 0x00];
        d.extend(std::iter::repeat(0xab).take(256));
        let mut r = Reader::new(&d);
        let v = r.next(0x04).unwrap();
        assert_eq!(v.len(), 256);
        assert_eq!(v[0], 0xab);
        assert!(r.next(0x04).is_none());
    }

    #[test]
    fn int_stripping() {
        assert_eq!(int_bytes(&[0, 0, 5, 6]), &[5, 6]);
        assert_eq!(int_bytes(&[7]), &[7]);
        assert_eq!(int_bytes(&[0, 0]), &[0]);
        assert_eq!(OID_RSA.len(), 9);
    }
}
