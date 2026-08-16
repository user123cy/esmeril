use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const TTL: Duration = Duration::from_secs(3600);

pub fn fetch_cached(agent: &ureq::Agent, url: &str, offline: bool) -> Result<String, String> {
    fetch_cached_in(&cache_dir(), agent, url, offline)
}

fn fetch_cached_in(
    dir: &Path,
    agent: &ureq::Agent,
    url: &str,
    offline: bool,
) -> Result<String, String> {
    let path = dir.join(key(url));
    let cached = std::fs::read_to_string(&path).ok();
    if offline {
        return cached.ok_or_else(|| "offline and no cached copy".to_string());
    }
    if let Some(text) = &cached
        && is_fresh(&path)
    {
        return Ok(text.clone());
    }
    match crate::deps::fetch(agent, url) {
        Ok(text) => {
            let _ = std::fs::create_dir_all(dir);
            let _ = std::fs::write(&path, &text);
            Ok(text)
        }
        Err(e) => cached.ok_or(e),
    }
}

fn cache_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("ESMERIL_CACHE") {
        return PathBuf::from(dir);
    }
    #[cfg(windows)]
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local).join("esmeril").join("cache");
    }
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(xdg).join("esmeril");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".cache").join("esmeril");
    }
    PathBuf::from(".")
}

fn is_fresh(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    SystemTime::now()
        .duration_since(modified)
        .map(|age| age <= TTL)
        .unwrap_or(false)
}

fn key(url: &str) -> String {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_deterministic_and_distinct() {
        assert_eq!(key("https://x/1"), key("https://x/1"));
        assert_ne!(key("https://x/1"), key("https://x/2"));
    }

    #[test]
    fn offline_serves_the_cache() {
        let dir = std::env::temp_dir().join(format!("esmeril-cache-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let url = "https://raw.githubusercontent.com/example/index/main/a/b";
        std::fs::write(dir.join(key(url)), "cached-body").unwrap();
        let agent = crate::deps::agent();
        assert_eq!(
            fetch_cached_in(&dir, &agent, url, true).unwrap(),
            "cached-body"
        );
        assert!(fetch_cached_in(&dir, &agent, "https://other/url", true).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
