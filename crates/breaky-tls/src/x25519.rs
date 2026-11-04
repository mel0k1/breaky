use crate::field::{Mont, Uint};

const P: Uint<4> = Uint([
    0xffffffffffffffed,
    0xffffffffffffffff,
    0xffffffffffffffff,
    0x7fffffffffffffff,
]);

fn clamp(mut k: [u8; 32]) -> [u8; 32] {
    k[0] &= 248;
    k[31] &= 127;
    k[31] |= 64;
    k
}

fn cswap(swap: u8, a: &mut Uint<4>, b: &mut Uint<4>) {
    let mask = 0u64.wrapping_sub(swap as u64);
    for i in 0..4 {
        let d = a.0[i] ^ b.0[i];
        let t = mask & d;
        a.0[i] ^= t;
        b.0[i] ^= t;
    }
}

pub fn scalar_mult(scalar: [u8; 32], u: [u8; 32]) -> [u8; 32] {
    let f = Mont::new(P);
    let mut ub = u;
    ub[31] &= 127;
    let mut x1 = Uint::from_le32(&ub);
    if x1.ge(&P) {
        x1 = x1.sub_wrap(&P).0;
    }
    let x1m = f.to_mont(x1);
    let one = f.to_mont(Uint::one());
    let zero = f.to_mont(Uint::zero());
    let a24 = f.to_mont(Uint::from_u64(121665));
    let (mut x2, mut z2) = (one, zero);
    let (mut x3, mut z3) = (x1m, one);
    let s = clamp(scalar);
    let mut swap = 0u8;
    for t in (0..255).rev() {
        let kt = (s[t / 8] >> (t % 8)) & 1;
        swap ^= kt;
        cswap(swap, &mut x2, &mut x3);
        cswap(swap, &mut z2, &mut z3);
        swap = kt;
        let a = f.add(x2, z2);
        let aa = f.mul(a, a);
        let b = f.sub(x2, z2);
        let bb = f.mul(b, b);
        let e = f.sub(aa, bb);
        let c = f.add(x3, z3);
        let d = f.sub(x3, z3);
        let da = f.mul(d, a);
        let cb = f.mul(c, b);
        x3 = f.mul(f.add(da, cb), f.add(da, cb));
        z3 = f.mul(x1m, f.mul(f.sub(da, cb), f.sub(da, cb)));
        x2 = f.mul(aa, bb);
        z2 = f.mul(e, f.add(aa, f.mul(a24, e)));
    }
    cswap(swap, &mut x2, &mut x3);
    cswap(swap, &mut z2, &mut z3);
    let z2p = f.from_mont(z2);
    let zinv = f.inv(z2p);
    let out = f.mul(x2, f.to_mont(zinv));
    f.from_mont(out).to_le32()
}

pub fn base_mult(scalar: [u8; 32]) -> [u8; 32] {
    let mut u = [0u8; 32];
    u[0] = 9;
    scalar_mult(scalar, u)
}

pub fn shared(priv_key: [u8; 32], peer: [u8; 32]) -> [u8; 32] {
    scalar_mult(priv_key, peer)
}

#[cfg(test)]
mod tests {
    use super::{base_mult, scalar_mult, shared};

    fn hex(d: &[u8]) -> String {
        d.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn rfc7748_5_2_vectors() {
        let out = scalar_mult(
            unhex("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4")
                .try_into()
                .unwrap(),
            unhex("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c")
                .try_into()
                .unwrap(),
        );
        assert_eq!(
            hex(&out),
            "c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552"
        );
        let out = scalar_mult(
            unhex("4b66e9d4d1b4673c5ad22691957d6af5c11b6421e0ea01d42ca4169e7918ba0d")
                .try_into()
                .unwrap(),
            unhex("e5210f12786811d3f4b7959d0538ae2c31dbe7106fc03c3efc4cd549c715a493")
                .try_into()
                .unwrap(),
        );
        assert_eq!(
            hex(&out),
            "95cbde9476e8907d7aade45cb4b873f88b595a68799fa152e6f8f7647aac7957"
        );
    }

    #[test]
    fn rfc7748_diffie_hellman() {
        let alice_priv = unhex("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a")
            .try_into()
            .unwrap();
        let alice_pub = base_mult(alice_priv);
        assert_eq!(
            hex(&alice_pub),
            "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"
        );
        let bob_priv = unhex("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb")
            .try_into()
            .unwrap();
        let bob_pub = base_mult(bob_priv);
        assert_eq!(
            hex(&bob_pub),
            "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f"
        );
        assert_eq!(
            hex(&shared(alice_priv, bob_pub)),
            "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742"
        );
        assert_eq!(
            hex(&shared(bob_priv, alice_pub)),
            "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742"
        );
    }
}
