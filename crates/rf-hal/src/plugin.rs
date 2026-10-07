//! Supervised persistent Python adapter. The protocol thread owns pipe I/O;
//! deadlines kill/reap the helper even when a vendor call or pipe blocks.
use crate::{Capabilities, Result};
use rf_core::dsp::{IqFrame, Settings};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
const ADAPTER: &str = include_str!("../../../python/adapters/hal_adapter.py");
pub struct Plugin {
    child: Child,
    tx: mpsc::SyncSender<Value>,
    rx: mpsc::Receiver<Result<Value>>,
    timeout: Duration,
    caps: Capabilities,
    healthy: bool,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Plugin {
    pub fn open(c: &Settings, source: bool) -> Result<Self> {
        let mut cmd = Command::new(&c.python_executable);
        cmd.args(["-u", "-c", ADAPTER])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Python HAL indisponible : {e}"))?;
        let mut input = child.stdin.take().ok_or("stdin absent")?;
        let output = child.stdout.take().ok_or("stdout absent")?;
        let (tx, requests) = mpsc::sync_channel::<Value>(1);
        let (responses, rx) = mpsc::sync_channel(1);
        let thread = std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            for v in requests {
                let result = (|| {
                    writeln!(input, "{v}").map_err(|e| e.to_string())?;
                    let mut line = String::new();
                    let n = (&mut reader)
                        .take(4_000_001)
                        .read_line(&mut line)
                        .map_err(|e| e.to_string())?;
                    if n == 0 || n > 4_000_000 || !line.ends_with('\n') {
                        return Err("Réponse HAL absente/invalide/trop grande".into());
                    }
                    serde_json::from_str(&line).map_err(|e| e.to_string())
                })();
                if responses.send(result).is_err() {
                    break;
                }
            }
        });
        let mut p = Self {
            child,
            tx,
            rx,
            timeout: Duration::from_millis(c.timeout_ms),
            caps: Capabilities::default(),
            healthy: true,
            thread: Some(thread),
        };
        let r = p.call(json!({"op":"open","settings":c,"source":source}))?;
        p.caps = serde_json::from_value(r["capabilities"].clone()).map_err(|e| e.to_string())?;
        p.caps.check(c)?;
        Ok(p)
    }
    fn call(&mut self, v: Value) -> Result<Value> {
        if !self.healthy {
            return Err("HAL : protocole fermé après erreur/délai".into());
        }
        if let Err(e) = self.tx.try_send(v) {
            self.healthy = false;
            return Err(e.to_string());
        }
        let r = match self.rx.recv_timeout(self.timeout) {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                self.healthy = false;
                return Err(e);
            }
            Err(e) => {
                self.healthy = false;
                return Err(format!("HAL : délai/connexion : {e}"));
            }
        };
        if let Some(e) = r["error"].as_str() {
            return Err(e.into());
        }
        Ok(r)
    }
    pub fn read(&mut self) -> Result<IqFrame> {
        let r = self.call(json!({"op":"read"}))?;
        let f: IqFrame = serde_json::from_value(r["frame"].clone()).map_err(|e| e.to_string())?;
        f.validate()?;
        Ok(f)
    }
    pub fn write(&mut self, f: &IqFrame) -> Result<()> {
        self.call(json!({"op":"write","frame":f})).map(|_| ())
    }
}
impl Drop for Plugin {
    fn drop(&mut self) {
        if self.healthy {
            let _ = self.call(json!({"op":"close"}));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let (dummy, _) = mpsc::sync_channel(1);
        let original = std::mem::replace(&mut self.tx, dummy);
        drop(original);
        if let Some(h) = self.thread.take() {
            let _ = h.join();
        }
    }
}
