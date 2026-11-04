use crate::pem;
use crate::x509::{self, Cert};
use std::sync::OnceLock;

const PATHS: [&str; 4] = [
    "/etc/ssl/certs/ca-certificates.crt",
    "/etc/pki/tls/certs/ca-bundle.crt",
    "/etc/pki/tls/cert.pem",
    "/etc/ssl/cert.pem",
];

pub fn load_from(path: &str) -> Result<Vec<Cert>, &'static str> {
    let text = std::fs::read_to_string(path).map_err(|_| "cannot read trust store")?;
    let mut roots = Vec::new();
    for der in pem::certificates(&text) {
        if let Some(cert) = x509::parse(&der) {
            roots.push(cert);
        }
    }
    if roots.is_empty() {
        return Err("no certificates in trust store");
    }
    Ok(roots)
}

pub fn load() -> Result<&'static [Cert], &'static str> {
    static ROOTS: OnceLock<Result<Vec<Cert>, &'static str>> = OnceLock::new();
    let r = ROOTS.get_or_init(|| {
        for path in PATHS {
            if std::path::Path::new(path).exists() {
                if let Ok(roots) = load_from(path) {
                    return Ok(roots);
                }
            }
        }
        Err("no system trust store found")
    });
    match r {
        Ok(v) => Ok(v.as_slice()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::{load_from, PATHS};
    use crate::x509::SigAlg;
    use std::path::Path;

    #[test]
    fn system_bundle_loads() {
        let path = PATHS
            .iter()
            .find(|p| Path::new(p).exists())
            .expect("no system trust store on this machine");
        let roots = load_from(path).unwrap();
        assert!(roots.len() > 50);
        assert!(roots
            .iter()
            .any(|r| matches!(r.key, crate::x509::PubKey::Rsa { .. })));
        assert!(roots
            .iter()
            .any(|r| matches!(r.key, crate::x509::PubKey::EcP256 { .. })));
    }

    #[test]
    fn missing_store_errors() {
        assert!(matches!(
            load_from("/nonexistent/bundle.pem"),
            Err("cannot read trust store")
        ));
        let empty = std::env::temp_dir().join("breaky-empty-store.pem");
        std::fs::write(&empty, "nothing here\n").unwrap();
        assert!(matches!(
            load_from(empty.to_str().unwrap()),
            Err("no certificates in trust store")
        ));
    }

    #[test]
    fn sig_algs_seen_in_wild() {
        let path = PATHS
            .iter()
            .find(|p| Path::new(p).exists())
            .expect("no system trust store on this machine");
        let roots = load_from(path).unwrap();
        let algs: Vec<SigAlg> = roots.iter().map(|r| r.sig_alg).collect();
        assert!(algs.contains(&SigAlg::RsaPkcs1Sha256));
    }
}
