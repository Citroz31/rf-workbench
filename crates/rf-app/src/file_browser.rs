//! Directory listing runs outside the render thread (including network shares).
use std::{
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};
type Listing = Result<Vec<(PathBuf, bool)>, String>;
pub struct Browser {
    pub entries: Vec<(PathBuf, bool)>,
    pub error: Option<String>,
    requested: String,
    loaded: String,
    changed: Instant,
    rx: Option<mpsc::Receiver<(String, Listing)>>,
}
impl Default for Browser {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            error: None,
            requested: String::new(),
            loaded: String::new(),
            changed: Instant::now(),
            rx: None,
        }
    }
}
impl Browser {
    pub fn busy(&self) -> bool {
        self.loaded != self.requested || self.rx.is_some()
    }
    pub fn update(&mut self, directory: &str) -> bool {
        if self.requested != directory {
            self.requested = directory.into();
            self.changed = Instant::now();
            self.entries.clear();
            self.error = None;
        }
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok((path, result)) => {
                    self.rx = None;
                    if path == self.requested {
                        self.loaded = path;
                        match result {
                            Ok(v) => self.entries = v,
                            Err(e) => self.error = Some(e),
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.rx = None;
                    self.loaded = self.requested.clone();
                    self.error = Some("Lecture du dossier interrompue".into());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.rx.is_none()
            && self.loaded != self.requested
            && self.changed.elapsed() >= Duration::from_millis(250)
        {
            let path = self.requested.clone();
            let (tx, rx) = mpsc::channel();
            self.rx = Some(rx);
            std::thread::spawn(move || {
                let result = std::fs::read_dir(&path)
                    .map_err(|e| e.to_string())
                    .map(|entries| {
                        let mut paths: Vec<_> = entries
                            .take(10000)
                            .filter_map(Result::ok)
                            .map(|e| {
                                let p = e.path();
                                let dir = p.is_dir();
                                (p, dir)
                            })
                            .filter(|(p, dir)| {
                                *dir || p.extension().is_some_and(|x| {
                                    x.eq_ignore_ascii_case("rfbench")
                                        || x.eq_ignore_ascii_case("json")
                                })
                            })
                            .take(200)
                            .collect();
                        paths.sort();
                        paths
                    });
                let _ = tx.send((path, result));
            });
        }
        self.loaded != self.requested || self.rx.is_some()
    }
}
