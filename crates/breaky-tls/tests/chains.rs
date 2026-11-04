use tls::{chain, pem, roots, x509};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

fn certs(names: &[&str]) -> Vec<x509::Cert> {
    names
        .iter()
        .map(|n| {
            let text =
                std::fs::read_to_string(format!("{DIR}/{n}.pem")).expect("fixture missing");
            let der = pem::certificates(&text);
            assert_eq!(der.len(), 1, "{n}");
            x509::parse(&der[0]).unwrap_or_else(|| panic!("unparsable {n}"))
        })
        .collect()
}

fn mid_time(c: &x509::Cert) -> i64 {
    (c.not_before + c.not_after) / 2
}

#[test]
fn fixture_sig_algs() {
    let leaf = &certs(&["leaf-256"])[0];
    let inter = &certs(&["inter-ec"])[0];
    let root = &certs(&["root-rsa"])[0];
    let mini = &certs(&["root-mini"])[0];
    assert_eq!(leaf.sig_alg, x509::SigAlg::EcdsaSha256);
    assert_eq!(inter.sig_alg, x509::SigAlg::RsaPkcs1Sha384);
    assert_eq!(root.sig_alg, x509::SigAlg::RsaPkcs1Sha256);
    assert_eq!(mini.sig_alg, x509::SigAlg::RsaPkcs1Sha512);
    assert!(inter.ca);
    assert!(inter.key_cert_sign);
    assert_eq!(leaf.dns_names, vec!["example.org".to_string()]);
    assert_eq!(leaf.issuer, inter.subject);
    assert_eq!(inter.issuer, root.subject);
    assert_ne!(leaf.key, root.key);
    assert!(matches!(inter.key, x509::PubKey::EcP384 { .. }));
    assert!(matches!(leaf.key, x509::PubKey::EcP256 { .. }));
}

#[test]
fn good_chain_ec_p256() {
    let leaf = certs(&["leaf-256"]);
    let inter = certs(&["inter-ec"]);
    let roots = certs(&["root-rsa"]);
    let mut full = leaf.clone();
    full.extend(inter);
    let now = mid_time(&leaf[0]);
    assert_eq!(chain::validate(&full, &roots, "example.org", now), Ok(()));
    assert_eq!(
        chain::validate(&full, &roots, "www.example.org", now),
        Err("hostname mismatch")
    );
}

#[test]
fn good_chain_ec_p384() {
    let leaf = certs(&["leaf-384"]);
    let inter = certs(&["inter-ec"]);
    let roots = certs(&["root-rsa"]);
    let mut full = leaf.clone();
    full.extend(inter);
    let now = mid_time(&leaf[0]);
    assert_eq!(chain::validate(&full, &roots, "example.net", now), Ok(()));
}

#[test]
fn good_chain_rsa() {
    let leaf = certs(&["leaf-rsa"]);
    let inter = certs(&["inter-rsa"]);
    let roots = certs(&["root-rsa"]);
    let mut full = leaf.clone();
    full.extend(inter);
    let now = mid_time(&leaf[0]);
    assert_eq!(chain::validate(&full, &roots, "leaf.test", now), Ok(()));
}

#[test]
fn direct_anchor() {
    let leaf = certs(&["leaf-direct"]);
    let roots = certs(&["root-rsa"]);
    let now = mid_time(&leaf[0]);
    assert_eq!(chain::validate(&leaf, &roots, "direct.test", now), Ok(()));
}

#[test]
fn cert_without_san_fails_hostname() {
    let inter = certs(&["inter-ec"]);
    let roots = certs(&["root-rsa"]);
    let now = mid_time(&inter[0]);
    assert_eq!(
        chain::validate(&inter, &roots, "anything.test", now),
        Err("hostname mismatch")
    );
}

#[test]
fn expired_and_not_yet_valid() {
    let leaf = certs(&["leaf-256"]);
    let inter = certs(&["inter-ec"]);
    let roots = certs(&["root-rsa"]);
    let mut full = leaf.clone();
    full.extend(inter);
    assert_eq!(
        chain::validate(&full, &roots, "example.org", leaf[0].not_after + 1),
        Err("expired")
    );
    assert_eq!(
        chain::validate(&full, &roots, "example.org", leaf[0].not_before - 1),
        Err("not yet valid")
    );
}

#[test]
fn intermediate_expired() {
    let leaf = certs(&["leaf-256"]);
    let inter = certs(&["inter-ec"]);
    let roots = certs(&["root-rsa"]);
    let mut full = leaf.clone();
    full.extend(inter);
    let now = full[1].not_after + 1;
    assert_eq!(
        chain::validate(&full, &roots, "example.org", now),
        Err("expired")
    );
}

#[test]
fn tampered_leaf_fails_signature() {
    let text =
        std::fs::read_to_string(format!("{DIR}/leaf-256.pem")).unwrap();
    let mut der = pem::certificates(&text).remove(0);
    let now = mid_time(&certs(&["leaf-256"])[0]);
    let needle = [0x02u8, 0x02, 0x03, 0xeb];
    let pos = der
        .windows(4)
        .position(|w| w == needle)
        .expect("serial bytes");
    der[pos + 3] ^= 0x01;
    let leaf = x509::parse(&der).expect("tampered cert still parses");
    let mut full = vec![leaf];
    full.extend(certs(&["inter-ec"]));
    let roots = certs(&["root-rsa"]);
    assert_eq!(
        chain::validate(&full, &roots, "example.org", now),
        Err("signature invalid")
    );
}

#[test]
fn unanchored_and_missing_intermediate() {
    let leaf = certs(&["leaf-256"]);
    let inter = certs(&["inter-ec"]);
    let wrong_roots = certs(&["root-mini"]);
    let right_roots = certs(&["root-rsa"]);
    let mut full = leaf.clone();
    full.extend(inter);
    let now = mid_time(&leaf[0]);
    assert_eq!(
        chain::validate(&full, &wrong_roots, "example.org", now),
        Err("no issuer for certificate")
    );
    assert_eq!(
        chain::validate(&leaf, &right_roots, "example.org", now),
        Err("no issuer for certificate")
    );
    assert_eq!(
        chain::validate(&full, &[], "example.org", now),
        Err("no issuer for certificate")
    );
}

#[test]
fn non_ca_issuer_rejected() {
    let child = certs(&["nonca-child"]);
    let nonca = certs(&["nonca"]);
    let mut full = child.clone();
    full.extend(nonca);
    let roots = certs(&["root-rsa"]);
    let now = mid_time(&child[0]);
    assert_eq!(
        chain::validate(&full, &roots, "child.example", now),
        Err("issuer is not a ca")
    );
}

#[test]
fn path_len_zero_violated() {
    let leaf = certs(&["leaf-pathlen"]);
    let inter = certs(&["inter-pathlen"]);
    let roots = certs(&["root-pathlen"]);
    let mut full = leaf.clone();
    full.extend(inter);
    let now = mid_time(&leaf[0]);
    assert_eq!(
        chain::validate(&full, &roots, "leaf.example", now),
        Err("path length exceeded")
    );
}

#[test]
fn bundle_loading() {
    let mut bundle = String::new();
    for n in ["root-rsa", "root-mini"] {
        bundle.push_str(&std::fs::read_to_string(format!("{DIR}/{n}.pem")).unwrap());
    }
    let path = std::env::temp_dir().join("breaky-test-bundle.pem");
    std::fs::write(&path, bundle).unwrap();
    let loaded = roots::load_from(path.to_str().unwrap()).unwrap();
    assert_eq!(loaded.len(), 2);
    let mini_only: Vec<x509::Cert> = loaded
        .iter()
        .filter(|c| !matches!(c.sig_alg, x509::SigAlg::RsaPkcs1Sha256))
        .cloned()
        .collect();
    assert_eq!(mini_only.len(), 1);
    let leaf = certs(&["leaf-256"]);
    let inter = certs(&["inter-ec"]);
    let mut full = leaf.clone();
    full.extend(inter);
    let now = mid_time(&leaf[0]);
    assert_eq!(chain::validate(&full, &loaded, "example.org", now), Ok(()));
    assert_eq!(
        chain::validate(&full, &mini_only, "example.org", now),
        Err("no issuer for certificate")
    );
    assert!(matches!(
        roots::load_from("/nonexistent/bundle.pem"),
        Err("cannot read trust store")
    ));
}

#[test]
fn system_roots_are_cas() {
    let path = "/etc/ssl/certs/ca-certificates.crt";
    if !std::path::Path::new(path).exists() {
        return;
    }
    let store = roots::load_from(path).unwrap();
    assert!(store.len() > 50);
    assert!(store.iter().filter(|r| r.ca).count() > store.len() / 2);
    assert!(store
        .iter()
        .any(|r| matches!(r.key, x509::PubKey::Rsa { .. })));
    assert!(store
        .iter()
        .any(|r| matches!(r.key, x509::PubKey::EcP256 { .. })));
    assert!(store
        .iter()
        .any(|r| matches!(r.sig_alg, x509::SigAlg::RsaPkcs1Sha256 | x509::SigAlg::EcdsaSha256)));
}
