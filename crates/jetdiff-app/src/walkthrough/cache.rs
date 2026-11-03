//! Finished walkthroughs, by the exact prompt and model they were written with. Putting the code
//! back as it was brings the earlier walkthrough back without asking the model again, and a changed
//! prompt writes a new one.

use std::fs;
use std::path::PathBuf;

/// The newest are kept.
const MAX_ENTRIES: usize = 200;

pub struct Cache {
    folder: PathBuf,
}

impl Cache {
    /// `$XDG_CACHE_HOME/jetdiff/walkthroughs`, else `~/.cache/jetdiff/walkthroughs`.
    pub fn new() -> Option<Self> {
        let base = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
        Some(Self::in_folder(base.join("jetdiff/walkthroughs")))
    }

    fn in_folder(folder: PathBuf) -> Self {
        Self { folder }
    }

    pub fn read(&self, key: &str) -> Option<String> {
        fs::read_to_string(self.path(key)).ok()
    }

    pub fn write(&self, key: &str, answer: &str) -> Result<(), String> {
        fs::create_dir_all(&self.folder).map_err(|e| e.to_string())?;
        fs::write(self.path(key), answer).map_err(|e| e.to_string())?;
        self.keep_newest();
        Ok(())
    }

    fn path(&self, key: &str) -> PathBuf {
        self.folder.join(format!("{key}.json"))
    }

    fn keep_newest(&self) {
        let Ok(entries) = fs::read_dir(&self.folder) else {
            return;
        };
        let mut entries: Vec<_> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
            .collect();
        entries.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
        for (_, path) in entries.into_iter().skip(MAX_ENTRIES) {
            let _ = fs::remove_file(path);
        }
    }
}

pub fn cache_key(parts: &[&str]) -> String {
    let mut hash = Fnv::default();
    for part in parts {
        hash.write(part.as_bytes());
        // Keeps "ab" + "c" apart from "a" + "bc"
        hash.write(&[0]);
    }
    format!("{:016x}", hash.0)
}

/// FNV-1a, stable across Rust versions unlike `DefaultHasher`.
struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Fnv {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_follows_every_part() {
        let key = cache_key(&["haiku", "=== a.rs"]);
        assert_eq!(key.len(), 16);
        assert_eq!(key, cache_key(&["haiku", "=== a.rs"]));
        assert_ne!(key, cache_key(&["opus", "=== a.rs"]));
        assert_ne!(cache_key(&["ab", "c"]), cache_key(&["a", "bc"]));
    }

    #[test]
    fn fnv_1a_of_a_known_value() {
        let mut hash = Fnv::default();
        hash.write(b"a");
        assert_eq!(hash.0, 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn written_answers_are_read_back() {
        let folder = std::env::temp_dir().join(format!("jetdiff-cache-{}", std::process::id()));
        let cache = Cache::in_folder(folder.clone());
        assert_eq!(cache.read("k"), None);
        cache.write("k", "{}").unwrap();
        assert_eq!(cache.read("k").as_deref(), Some("{}"));
        let _ = fs::remove_dir_all(folder);
    }
}
