use crate::sha256;
use std::collections::HashMap;
use std::sync::Mutex;

struct Entry {
    fingerprint: [u8; 32],
    not_after: i64,
}

pub struct ChainCache {
    entries: Mutex<HashMap<String, Entry>>,
}

impl ChainCache {
    pub fn new() -> ChainCache {
        ChainCache {
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub fn hit(&self, host: &str, tbs: &[u8], now: i64) -> bool {
        let fp = sha256::hash(tbs);
        match self.entries.lock() {
            Ok(g) => match g.get(host) {
                Some(e) => e.fingerprint == fp && now <= e.not_after,
                None => false,
            },
            Err(_) => false,
        }
    }

    pub fn remember(&self, host: &str, tbs: &[u8], not_after: i64) {
        let fp = sha256::hash(tbs);
        if let Ok(mut g) = self.entries.lock() {
            g.insert(
                host.to_string(),
                Entry {
                    fingerprint: fp,
                    not_after,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ChainCache;

    #[test]
    fn miss_then_hit() {
        let c = ChainCache::new();
        assert!(!c.hit("a.example", &[1, 2, 3], 100));
        c.remember("a.example", &[1, 2, 3], 200);
        assert!(c.hit("a.example", &[1, 2, 3], 200));
        assert!(c.hit("a.example", &[1, 2, 3], 100));
    }

    #[test]
    fn expires_after_not_after() {
        let c = ChainCache::new();
        c.remember("a.example", &[1, 2, 3], 200);
        assert!(!c.hit("a.example", &[1, 2, 3], 201));
    }

    #[test]
    fn rejects_other_host_or_cert() {
        let c = ChainCache::new();
        c.remember("a.example", &[1, 2, 3], 200);
        assert!(!c.hit("b.example", &[1, 2, 3], 100));
        assert!(!c.hit("a.example", &[9, 9, 9], 100));
        c.remember("a.example", &[4, 5, 6], 300);
        assert!(c.hit("a.example", &[4, 5, 6], 300));
        assert!(!c.hit("a.example", &[1, 2, 3], 300));
    }
}
