use crate::field::{Mont, Uint};

const P: Uint<4> = Uint([
    0xffffffffffffffff,
    0x00000000ffffffff,
    0x0000000000000000,
    0xffffffff00000001,
]);
const N: Uint<4> = Uint([
    0xf3b9cac2fc632551,
    0xbce6faada7179e84,
    0xffffffffffffffff,
    0xffffffff00000000,
]);
const GY0: Uint<4> = Uint([
    0xcbb6406837bf51f5,
    0x2bce33576b315ece,
    0x8ee7eb4a7c0f9e16,
    0x4fe342e2fe1a7f9b,
]);
const GX0: Uint<4> = Uint([
    0xf4a13945d898c296,
    0x77037d812deb33a0,
    0xf8bce6e563a440f2,
    0x6b17d1f2e12c4247,
]);
const A: Uint<4> = Uint([
    0xfffffffffffffffc,
    0x00000000ffffffff,
    0x0000000000000000,
    0xffffffff00000001,
]);

#[derive(Clone, Copy)]
struct Point {
    x: Uint<4>,
    y: Uint<4>,
}

fn minv(f: &Mont<4>, v: Uint<4>) -> Uint<4> {
    f.to_mont(f.inv(f.from_mont(v)))
}

fn double(f: &Mont<4>, p: Point) -> Option<Point> {
    let three = f.to_mont(Uint::from_u64(3));
    let two = f.to_mont(Uint::from_u64(2));
    let mut num = f.mul(p.x, p.x);
    num = f.mul(num, three);
    num = f.add(num, f.to_mont(A));
    let den = f.mul(two, p.y);
    if den.is_zero() {
        return None;
    }
    let lam = f.mul(num, minv(f, den));
    let lam2 = f.mul(lam, lam);
    let x3 = f.sub(f.sub(lam2, p.x), p.x);
    let y3 = f.sub(f.mul(lam, f.sub(p.x, x3)), p.y);
    Some(Point { x: x3, y: y3 })
}

fn add(f: &Mont<4>, a: Point, b: Point) -> Option<Point> {
    if a.x == b.x {
        if a.y == b.y {
            return double(f, a);
        }
        return None;
    }
    let num = f.sub(b.y, a.y);
    let den = f.sub(b.x, a.x);
    let lam = f.mul(num, minv(f, den));
    let lam2 = f.mul(lam, lam);
    let x3 = f.sub(f.sub(lam2, a.x), b.x);
    let y3 = f.sub(f.mul(lam, f.sub(a.x, x3)), a.y);
    Some(Point { x: x3, y: y3 })
}

fn scalar_mult(f: &Mont<4>, k: Uint<4>, p: Point) -> Option<Point> {
    let mut acc: Option<Point> = None;
    for i in (0..256).rev() {
        if let Some(cur) = acc {
            acc = double(f, cur);
        }
        if k.bit(i) == 1 {
            acc = match acc {
                None => Some(p),
                Some(cur) => add(f, cur, p),
            };
        }
    }
    acc
}

fn reduce_n(v: Uint<4>) -> Uint<4> {
    if v.ge(&N) {
        v.sub_wrap(&N).0
    } else {
        v
    }
}

pub fn verify_ecdsa_p256_sha256(
    msg_hash: &[u8; 32],
    qx_bytes: &[u8],
    qy_bytes: &[u8],
    r_bytes: &[u8],
    s_bytes: &[u8],
) -> bool {
    if qx_bytes.len() > 32 || qy_bytes.len() > 32 || r_bytes.is_empty() || s_bytes.is_empty() {
        return false;
    }
    if r_bytes.len() > 32 || s_bytes.len() > 32 {
        return false;
    }
    let fp = Mont::new(P);
    let fnn = Mont::new(N);
    let qx = Uint::<4>::from_be(qx_bytes);
    let qy = Uint::<4>::from_be(qy_bytes);
    if qx.ge(&P) || qy.ge(&P) {
        return false;
    }
    let q = Point {
        x: fp.to_mont(qx),
        y: fp.to_mont(qy),
    };
    let r = reduce_n(Uint::<4>::from_be(r_bytes));
    let s = reduce_n(Uint::<4>::from_be(s_bytes));
    if r.is_zero() || s.is_zero() {
        return false;
    }
    if Uint::<4>::from_be(r_bytes).ge(&N) || Uint::<4>::from_be(s_bytes).ge(&N) {
        return false;
    }
    let mut e = Uint::<4>::from_be(msg_hash);
    if e.ge(&N) {
        e = e.sub_wrap(&N).0;
    }
    let w = fnn.inv(s);
    let u1 = fnn.from_mont(fnn.mul(fnn.to_mont(e), fnn.to_mont(w)));
    let u2 = fnn.from_mont(fnn.mul(fnn.to_mont(r), fnn.to_mont(w)));
    let g = Point {
        x: fp.to_mont(GX0),
        y: fp.to_mont(GY0),
    };
    let p1 = match scalar_mult(&fp, u1, g) {
        Some(p) => p,
        None => return false,
    };
    let p2 = match scalar_mult(&fp, u2, q) {
        Some(p) => p,
        None => return false,
    };
    let sum = match add(&fp, p1, p2) {
        Some(p) => p,
        None => return false,
    };
    let vx = fp.from_mont(sum.x);
    let v = reduce_n(vx);
    v == r
}

#[cfg(test)]
mod tests {
    use super::verify_ecdsa_p256_sha256;
    use crate::sha256;

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn rfc6979_a_2_5() {
        let qx = unhex("60FED4BA255A9D31C961EB74C6356D68C049B8923B61FA6CE669622E60F29FB6");
        let qy = unhex("7903FE1008B8BC99A41AE9E95628BC64F2F1B20C2D7E9F5177A3C294D4462299");
        let r = unhex("EFD48B2AACB6A8FD1140DD9CD45E81D69D2C877B56AAF991C34D0EA84EAF3716");
        let s = unhex("F7CB1C942D657C41D436C7A1B6E29F65F3E900DBB9AFF4064DC4AB2F843ACDA8");
        let h = sha256::hash(b"sample");
        assert!(verify_ecdsa_p256_sha256(&h, &qx, &qy, &r, &s));
    }

    #[test]
    fn rejects_tampering() {
        let qx = unhex("60FED4BA255A9D31C961EB74C6356D68C049B8923B61FA6CE669622E60F29FB6");
        let qy = unhex("7903FE1008B8BC99A41AE9E95628BC64F2F1B20C2D7E9F5177A3C294D4462299");
        let r = unhex("EFD48B2AACB6A8FD1140DD9CD45E81D69D2C877B56AAF991C34D0EA84EAF3716");
        let s = unhex("F7CB1C942D657C41D436C7A1B6E29F65F3E900DBB9AFF4064DC4AB2F843ACDA8");
        let h = sha256::hash(b"sample");
        let mut bad_r = r.clone();
        bad_r[0] ^= 1;
        assert!(!verify_ecdsa_p256_sha256(&h, &qx, &qy, &bad_r, &s));
        let mut bad_s = s.clone();
        bad_s[10] ^= 1;
        assert!(!verify_ecdsa_p256_sha256(&h, &qx, &qy, &r, &bad_s));
        let bad_h = sha256::hash(b"samplf");
        assert!(!verify_ecdsa_p256_sha256(&bad_h, &qx, &qy, &r, &s));
        let mut bad_q = qx.clone();
        bad_q[0] ^= 1;
        assert!(!verify_ecdsa_p256_sha256(&h, &bad_q, &qy, &r, &s));
        assert!(!verify_ecdsa_p256_sha256(&h, &qx, &qy, &[], &s));
    }
}
