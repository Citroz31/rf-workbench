//! Recent files contain paths only; opening a file never restores runtime state.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Recent {
    pub path: String,
    pub name: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Registry {
    pub recent: Vec<Recent>,
}
impl Registry {
    pub fn read(path: &str) -> Self {
        std::fs::metadata(path)
            .ok()
            .filter(|m| m.len() <= 128000)
            .and_then(|_| std::fs::read_to_string(path).ok())
            .and_then(|s| serde_json::from_str::<Self>(&s).ok())
            .filter(|r| {
                r.recent.len() <= 20
                    && r.recent
                        .iter()
                        .all(|p| p.path.len() <= 4096 && p.name.len() <= 128)
            })
            .unwrap_or_default()
    }
    pub fn remember(&mut self, path: &str, name: &str) {
        let path = std::fs::canonicalize(path)
            .unwrap_or_else(|_| std::path::PathBuf::from(path))
            .to_string_lossy()
            .into_owned();
        self.recent.retain(|p| !p.path.eq_ignore_ascii_case(&path));
        self.recent.insert(
            0,
            Recent {
                path,
                name: name.into(),
            },
        );
        self.recent.truncate(20);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recency_is_bounded_and_deduplicated() {
        let mut r = Registry::default();
        for i in 0..30 {
            r.remember(&format!("missing-{i}.rfbench"), "Test");
        }
        r.remember("missing-10.rfbench", "A");
        assert_eq!(r.recent.len(), 20);
        assert_eq!(r.recent[0].name, "A");
        assert_eq!(
            r.recent
                .iter()
                .filter(|p| p.path.ends_with("missing-10.rfbench"))
                .count(),
            1
        );
    }
}
