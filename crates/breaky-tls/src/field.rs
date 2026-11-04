use core::cmp::Ordering;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Uint<const N: usize>(pub [u64; N]);

impl<const N: usize> Uint<N> {
    pub fn zero() -> Self {
        Self([0u64; N])
    }

    pub fn one() -> Self {
        let mut a = [0u64; N];
        a[0] = 1;
        Self(a)
    }

    pub fn from_u64(v: u64) -> Self {
        let mut a = [0u64; N];
        a[0] = v;
        Self(a)
    }

    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|l| *l == 0)
    }

    pub fn cmp_to(&self, o: &Self) -> Ordering {
        for i in (0..N).rev() {
            match self.0[i].cmp(&o.0[i]) {
                Ordering::Equal => continue,
                other => return other,
            }
        }
        Ordering::Equal
    }

    pub fn ge(&self, o: &Self) -> bool {
        self.cmp_to(o) != Ordering::Less
    }

    fn add_wrap(&self, o: &Self) -> (Self, u64) {
        let mut out = [0u64; N];
        let mut carry = 0u64;
        for i in 0..N {
            let (s, c) = self.0[i].overflowing_add(o.0[i]);
            let (s2, c2) = s.overflowing_add(carry);
            out[i] = s2;
            carry = (c || c2) as u64;
        }
        (Self(out), carry)
    }

    pub fn sub_wrap(&self, o: &Self) -> (Self, u64) {
        let mut out = [0u64; N];
        let mut borrow = 0u64;
        for i in 0..N {
            let (d, b) = self.0[i].overflowing_sub(o.0[i]);
            let (d2, b2) = d.overflowing_sub(borrow);
            out[i] = d2;
            borrow = (b || b2) as u64;
        }
        (Self(out), borrow)
    }

    pub fn add_mod(&self, o: &Self, m: &Self) -> Self {
        let (s, carry) = self.add_wrap(o);
        if carry == 1 || s.ge(m) {
            s.sub_wrap(m).0
        } else {
            s
        }
    }

    pub fn sub_mod(&self, o: &Self, m: &Self) -> Self {
        let (d, borrow) = self.sub_wrap(o);
        if borrow == 1 {
            d.add_wrap(m).0
        } else {
            d
        }
    }

    pub fn shl1_mod(&self, m: &Self) -> Self {
        let mut out = [0u64; N];
        let mut carry = 0u64;
        for i in 0..N {
            out[i] = (self.0[i] << 1) | carry;
            carry = self.0[i] >> 63;
        }
        let r = Self(out);
        if carry == 1 || r.ge(m) {
            r.sub_wrap(m).0
        } else {
            r
        }
    }

    pub fn from_be(b: &[u8]) -> Self {
        let mut out = [0u64; N];
        for (i, byte) in b.iter().rev().enumerate() {
            out[i / 8] |= (*byte as u64) << (8 * (i % 8));
        }
        Self(out)
    }

    pub fn to_be(&self, out: &mut [u8]) {
        for (i, byte) in out.iter_mut().rev().enumerate() {
            *byte = (self.0[i / 8] >> (8 * (i % 8))) as u8;
        }
    }

    pub fn from_le32(b: &[u8; 32]) -> Self {
        let mut out = [0u64; N];
        for i in 0..N.min(4) {
            let mut limb = [0u8; 8];
            limb.copy_from_slice(&b[i * 8..i * 8 + 8]);
            out[i] = u64::from_le_bytes(limb);
        }
        Self(out)
    }

    pub fn to_le32(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        for i in 0..N.min(4) {
            out[i * 8..i * 8 + 8].copy_from_slice(&self.0[i].to_le_bytes());
        }
        out
    }

    pub fn bit(&self, i: usize) -> u8 {
        ((self.0[i / 64] >> (i % 64)) & 1) as u8
    }
}

fn inv64(x: u64) -> u64 {
    let mut inv = 1u64;
    for _ in 0..6 {
        inv = inv.wrapping_mul(2u64.wrapping_sub(x.wrapping_mul(inv)));
    }
    inv
}

pub struct Mont<const N: usize> {
    pub m: Uint<N>,
    r2: Uint<N>,
    n0: u64,
}

impl<const N: usize> Mont<N> {
    pub fn new(m: Uint<N>) -> Self {
        let n0 = inv64(m.0[0]).wrapping_neg();
        let mut r2 = Uint::one();
        for _ in 0..128 * N {
            r2 = r2.shl1_mod(&m);
        }
        Mont { m, r2, n0 }
    }

    pub fn mul(&self, a: Uint<N>, b: Uint<N>) -> Uint<N> {
        let mut t = [0u64; N];
        let mut t1 = 0u64;
        let mut t2;
        for i in 0..N {
            let mut carry = 0u64;
            for j in 0..N {
                let acc = t[j] as u128 + (a.0[i] as u128) * (b.0[j] as u128) + carry as u128;
                t[j] = acc as u64;
                carry = (acc >> 64) as u64;
            }
            let acc = t1 as u128 + carry as u128;
            t1 = acc as u64;
            t2 = (acc >> 64) as u64;

            let mitem = t[0].wrapping_mul(self.n0);
            let acc = t[0] as u128 + (mitem as u128) * (self.m.0[0] as u128);
            let mut carry = (acc >> 64) as u64;
            for j in 1..N {
                let acc = t[j] as u128 + (mitem as u128) * (self.m.0[j] as u128) + carry as u128;
                t[j - 1] = acc as u64;
                carry = (acc >> 64) as u64;
            }
            let acc = t1 as u128 + carry as u128;
            t[N - 1] = acc as u64;
            t1 = t2 + ((acc >> 64) as u64);
        }
        if t1 == 1 {
            Uint(t).sub_wrap(&self.m).0
        } else {
            let (d, borrow) = Uint(t).sub_wrap(&self.m);
            if borrow == 0 {
                d
            } else {
                Uint(t)
            }
        }
    }

    pub fn add(&self, a: Uint<N>, b: Uint<N>) -> Uint<N> {
        a.add_mod(&b, &self.m)
    }

    pub fn sub(&self, a: Uint<N>, b: Uint<N>) -> Uint<N> {
        a.sub_mod(&b, &self.m)
    }

    pub fn to_mont(&self, a: Uint<N>) -> Uint<N> {
        self.mul(a, self.r2)
    }

    pub fn from_mont(&self, a: Uint<N>) -> Uint<N> {
        self.mul(a, Uint::one())
    }

    pub fn pow(&self, base: Uint<N>, e: &Uint<N>) -> Uint<N> {
        let x = self.to_mont(base);
        let mut acc = self.mul(Uint::one(), self.r2);
        let mut started = false;
        for i in (0..N).rev() {
            for b in (0..64).rev() {
                let bit = (e.0[i] >> b) & 1;
                if !started {
                    if bit == 0 {
                        continue;
                    }
                    started = true;
                }
                acc = self.mul(acc, acc);
                if bit == 1 {
                    acc = self.mul(acc, x);
                }
            }
        }
        if !started {
            return Uint::one();
        }
        self.from_mont(acc)
    }

    pub fn inv(&self, a: Uint<N>) -> Uint<N> {
        let mut e = self.m;
        e.0[0] -= 2;
        self.pow(a, &e)
    }
}

#[cfg(test)]
mod tests {
    use super::{Mont, Uint};

    const M: Uint<4> = Uint([
        0xffffffffffffffed,
        0xffffffffffffffff,
        0xffffffffffffffff,
        0x7fffffffffffffff,
    ]);

    fn n(v: u64) -> Uint<4> {
        Uint::from_u64(v)
    }

    fn mont() -> Mont<4> {
        Mont::new(M)
    }

    #[test]
    fn be_roundtrip() {
        let bytes: Vec<u8> = (1..=32).collect();
        let v = Uint::<4>::from_be(&bytes);
        let mut out = [0u8; 32];
        v.to_be(&mut out);
        assert_eq!(out, bytes[..]);
        assert_eq!(Uint::<4>::from_be(&[]), Uint::zero());
    }

    #[test]
    fn add_sub_inverse() {
        let f = mont();
        let a = n(0x1234_5678_9abc_def0);
        let b = n(0xdead_beef_cafe_babe);
        assert_eq!(f.sub(f.add(a, b), b), a);
        assert_eq!(f.sub(a, a), Uint::zero());
        let big = Uint([
            0xffffffffffffffff,
            0xffffffffffffffff,
            0xffffffffffffffff,
            0x0fffffffffffffff,
        ]);
        let r = f.add(big, big);
        assert!(r.ge(&n(0)) && !r.ge(&M));
    }

    #[test]
    fn mul_identity_and_commutativity() {
        let f = mont();
        let x = n(1234567);
        let y = Uint([
            0x0123456789abcdef,
            0xfedcba9876543210,
            0x13579bdf2468ace0,
            0x1f2e3d4c5b6a7988,
        ]);
        let xm = f.to_mont(x);
        let ym = f.to_mont(y);
        let one_m = f.to_mont(Uint::one());
        assert_eq!(f.mul(xm, one_m), xm);
        assert_eq!(f.mul(xm, ym), f.mul(ym, xm));
        assert_eq!(f.mul(f.mul(xm, ym), xm), f.mul(xm, f.mul(ym, xm)));
        assert_eq!(f.from_mont(xm), x);
    }

    #[test]
    fn inv_and_pow() {
        let f = mont();
        let x = n(0x0bad_c0de);
        let one_m = f.to_mont(Uint::one());
        let xi = f.inv(x);
        assert_eq!(f.mul(f.to_mont(x), f.to_mont(xi)), one_m);
        let xm = f.to_mont(x);
        assert_eq!(f.pow(x, &Uint::from_u64(2)), f.from_mont(f.mul(xm, xm)));
        assert_eq!(f.pow(x, &Uint::from_u64(0)), Uint::one());
        assert_eq!(f.pow(Uint::zero(), &Uint::from_u64(5)), Uint::zero());
        let mut e = M;
        e.0[0] -= 2;
        assert_eq!(f.pow(x, &e), f.inv(x));
    }

    #[test]
    fn shl1_mod_wrap() {
        let f = mont();
        let near = f.sub(Uint::zero(), Uint::one());
        let mut r = near;
        for _ in 0..64 {
            r = r.shl1_mod(&M);
        }
        assert!(!r.ge(&M));
        let one = Uint::<4>::one();
        let mut shifted = one;
        for _ in 0..255 {
            shifted = shifted.shl1_mod(&M);
        }
        assert_eq!(shifted, n(19));
    }
}
