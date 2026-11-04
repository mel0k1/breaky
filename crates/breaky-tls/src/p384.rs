use crate::field::{Mont, Uint};

const P: Uint<6> = Uint([
    0x00000000ffffffff,
    0xffffffff00000000,
    0xfffffffffffffffe,
    0xffffffffffffffff,
    0xffffffffffffffff,
    0xffffffffffffffff,
]);
const N: Uint<6> = Uint([
    0xecec196accc52973,
    0x581a0db248b0a77a,
    0xc7634d81f4372ddf,
    0xffffffffffffffff,
    0xffffffffffffffff,
    0xffffffffffffffff,
]);
const A: Uint<6> = Uint([
    0x00000000fffffffc,
    0xffffffff00000000,
    0xfffffffffffffffe,
    0xffffffffffffffff,
    0xffffffffffffffff,
    0xffffffffffffffff,
]);
#[cfg(test)]
const B: Uint<6> = Uint([
    0x2a85c8edd3ec2aef,
    0xc656398d8a2ed19d,
    0x0314088f5013875a,
    0x181d9c6efe814112,
    0x988e056be3f82d19,
    0xb3312fa7e23ee7e4,
]);
const GX: Uint<6> = Uint([
    0x3a545e3872760ab7,
    0x5502f25dbf55296c,
    0x59f741e082542a38,
    0x6e1d3b628ba79b98,
    0x8eb1c71ef320ad74,
    0xaa87ca22be8b0537,
]);
const GY: Uint<6> = Uint([
    0x7a431d7c90ea0e5f,
    0x0a60b1ce1d7e819d,
    0xe9da3113b5f0b8c0,
    0xf8f41dbd289a147c,
    0x5d9e98bf9292dc29,
    0x3617de4a96262c6f,
]);

#[derive(Clone, Copy, PartialEq)]
struct Point {
    x: Uint<6>,
    y: Uint<6>,
}

fn minv(f: &Mont<6>, v: Uint<6>) -> Uint<6> {
    f.to_mont(f.inv(f.from_mont(v)))
}

fn double(f: &Mont<6>, p: Point) -> Option<Point> {
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

fn add(f: &Mont<6>, a: Point, b: Point) -> Option<Point> {
    if a.x == b.x {
        if a.y == b.y {
            return double(f, a);
        }
        return None;
    }
    let num = f.sub(b.y, a.y);
    let den = f.sub(b.x, a.x);
    if den.is_zero() {
        return None;
    }
    let lam = f.mul(num, minv(f, den));
    let lam2 = f.mul(lam, lam);
    let x3 = f.sub(f.sub(lam2, a.x), b.x);
    let y3 = f.sub(f.mul(lam, f.sub(a.x, x3)), a.y);
    Some(Point { x: x3, y: y3 })
}

fn scalar_mult(f: &Mont<6>, k: Uint<6>, p: Point) -> Option<Point> {
    let mut acc: Option<Point> = None;
    for i in (0..384).rev() {
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

fn reduce_n(v: Uint<6>) -> Uint<6> {
    if v.ge(&N) {
        v.sub_wrap(&N).0
    } else {
        v
    }
}

pub fn verify_ecdsa(
    msg_hash: &[u8],
    qx_bytes: &[u8],
    qy_bytes: &[u8],
    r_bytes: &[u8],
    s_bytes: &[u8],
) -> bool {
    let h = if msg_hash.len() > 48 {
        &msg_hash[..48]
    } else {
        msg_hash
    };
    if qx_bytes.len() > 48 || qy_bytes.len() > 48 {
        return false;
    }
    if r_bytes.is_empty() || s_bytes.is_empty() || r_bytes.len() > 49 || s_bytes.len() > 49 {
        return false;
    }
    let fp = Mont::new(P);
    let fnn = Mont::new(N);
    let qx = Uint::<6>::from_be(qx_bytes);
    let qy = Uint::<6>::from_be(qy_bytes);
    if qx.ge(&P) || qy.ge(&P) {
        return false;
    }
    if qx.is_zero() || qy.is_zero() {
        return false;
    }
    if Uint::<6>::from_be(r_bytes).ge(&N) || Uint::<6>::from_be(s_bytes).ge(&N) {
        return false;
    }
    let r = reduce_n(Uint::<6>::from_be(r_bytes));
    let s = reduce_n(Uint::<6>::from_be(s_bytes));
    if r.is_zero() || s.is_zero() {
        return false;
    }
    let mut e = Uint::<6>::from_be(h);
    if e.ge(&N) {
        e = e.sub_wrap(&N).0;
    }
    let w = fnn.inv(s);
    let u1 = fnn.from_mont(fnn.mul(fnn.to_mont(e), fnn.to_mont(w)));
    let u2 = fnn.from_mont(fnn.mul(fnn.to_mont(r), fnn.to_mont(w)));
    let g = Point {
        x: fp.to_mont(GX),
        y: fp.to_mont(GY),
    };
    let q = Point {
        x: fp.to_mont(qx),
        y: fp.to_mont(qy),
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
    use super::{verify_ecdsa, A, B, GX, GY, P};
    use crate::field::Mont;
    use crate::sha512::sha384;

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn generator_on_curve() {
        let f = Mont::new(P);
        let x = f.to_mont(GX);
        let y = f.to_mont(GY);
        let b = f.to_mont(B);
        let y2 = f.mul(y, y);
        let x3 = f.mul(f.mul(x, x), x);
        let ax = f.mul(x, f.to_mont(A));
        let rhs = f.add(f.add(x3, ax), b);
        assert_eq!(y2, rhs);
    }

    #[test]
    fn openssl_vector() {
        let x = unhex("a501b271a9945a6c6c8a69f6459a81de8f035f0cf3ab149b93318f53fba45e6721f133c55804fa5280a9dfd87e185c78");
        let y = unhex("31a22cf4d7c373b27bc90fcfefa993f0adf7817f7a825e6539b42987fcd8d0709dfb48c5b8a8e39cf05cc46e5ee1fb6f");
        let r = unhex("b527a9c23ed72d70a0f47429fb33339902ae3cb01ba4546cd21d2d9c199d1116493b74f67c3f17365dd01f4e2636ceb6");
        let s = unhex("360f66b79620c09c7d39f49c5d3fe6ec7bc34642c9e054452feaad6a1fe7c278d6e1dfe721a79cc706c2d303acd8998f");
        let h = sha384(b"breaky p384");
        assert!(verify_ecdsa(&h, &x, &y, &r, &s));
    }

    #[test]
    fn rejects_tampering() {
        let x = unhex("a501b271a9945a6c6c8a69f6459a81de8f035f0cf3ab149b93318f53fba45e6721f133c55804fa5280a9dfd87e185c78");
        let y = unhex("31a22cf4d7c373b27bc90fcfefa993f0adf7817f7a825e6539b42987fcd8d0709dfb48c5b8a8e39cf05cc46e5ee1fb6f");
        let r = unhex("b527a9c23ed72d70a0f47429fb33339902ae3cb01ba4546cd21d2d9c199d1116493b74f67c3f17365dd01f4e2636ceb6");
        let s = unhex("360f66b79620c09c7d39f49c5d3fe6ec7bc34642c9e054452feaad6a1fe7c278d6e1dfe721a79cc706c2d303acd8998f");
        let h = sha384(b"breaky p384");
        let mut bad = r.clone();
        bad[5] ^= 1;
        assert!(!verify_ecdsa(&h, &x, &y, &bad, &s));
        let mut bads = s.clone();
        bads[5] ^= 1;
        assert!(!verify_ecdsa(&h, &x, &y, &r, &bads));
        let badh = sha384(b"breaky p383");
        assert!(!verify_ecdsa(&badh, &x, &y, &r, &s));
        let mut badq = x.clone();
        badq[0] ^= 1;
        assert!(!verify_ecdsa(&h, &badq, &y, &r, &s));
        assert!(!verify_ecdsa(&h, &x, &y, &[], &s));
        let long = vec![0u8; 64];
        assert!(!verify_ecdsa(&long, &x, &y, &r, &s));
    }
}
