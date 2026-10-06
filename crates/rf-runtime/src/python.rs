use super::Result;
use rf_core::Trace;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

const RUNNER: &str = include_str!("../../../python/rfworkbench/runner.py");
const MAX_MESSAGE: u64 = 4_000_000;

pub fn run(
    executable: &str,
    source: &str,
    trace: &Trace,
    hardware: bool,
    cancelled: &AtomicBool,
    mut instrument: impl FnMut(&str, &str, bool) -> Result<String>,
) -> Result<Trace> {
    trace.validate().map_err(|e| e.to_string())?;
    let mut command = Command::new(executable);
    command
        .args(["-u", "-c", RUNNER])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().map_err(|e| {
        format!("Python indisponible ({executable}) : {e}. Régler le chemin dans l'onglet Python.")
    })?;
    let result = (|| {
        let mut input = child.stdin.take().ok_or("Entrée Python absente")?;
        writeln!(
            input,
            "{}",
            json!({"source":source,"trace":trace,"hardware":hardware})
        )
        .map_err(|e| e.to_string())?;
        let output = child.stdout.take().ok_or("Sortie Python absente")?;
        let (tx, rx) = mpsc::sync_channel(8);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            for _ in 0..128 {
                let mut line = String::new();
                // Bound each protocol message, including malicious/malformed output.
                let read = (&mut reader).take(MAX_MESSAGE + 1).read_line(&mut line);
                match read {
                    Ok(0) => break,
                    Ok(n) if n as u64 <= MAX_MESSAGE && line.ends_with('\n') => {
                        if tx.send(Ok(line)).is_err() {
                            break;
                        }
                    }
                    _ => {
                        let _ =
                            tx.send(Err("Message Python invalide ou trop volumineux".to_string()));
                        break;
                    }
                }
            }
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut operations = 0;
        loop {
            if cancelled.load(Ordering::Relaxed) {
                return Err("Script Python arrêté".into());
            }
            if Instant::now() >= deadline {
                return Err("Script Python : délai de 5 s dépassé".into());
            }
            let line = match rx.recv_timeout(Duration::from_millis(20)) {
                Ok(v) => v?,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => return Err("Python a fermé la connexion sans résultat".into()),
            };
            let message: Value =
                serde_json::from_str(&line).map_err(|e| format!("Protocole Python : {e}"))?;
            match message["op"].as_str() {
                Some("result") => {
                    let mut output: Trace = serde_json::from_value(message["trace"].clone())
                        .map_err(|e| e.to_string())?;
                    output.simulated = trace.simulated; // Scripts cannot relabel simulated measurements as real.
                    output
                        .validate()
                        .map_err(|e| format!("Résultat Python : {e}"))?;
                    return Ok(output);
                }
                Some("error") => {
                    return Err(message["message"]
                        .as_str()
                        .unwrap_or("Erreur Python")
                        .to_string());
                }
                Some(op @ ("query" | "write")) => {
                    operations += 1;
                    if operations > 100 {
                        return Err("Trop de requêtes d'instruments dans le script".into());
                    }
                    let resource = message["resource"]
                        .as_str()
                        .ok_or("Ressource Python absente")?;
                    let cmd = message["command"]
                        .as_str()
                        .ok_or("Commande Python absente")?;
                    let response = match instrument(resource, cmd, op == "write") {
                        Ok(value) => json!({"ok":true,"value":value}),
                        Err(e) => json!({"ok":false,"error":e}),
                    };
                    writeln!(input, "{response}").map_err(|e| e.to_string())?;
                }
                _ => return Err("Opération Python inconnue".into()),
            }
        }
    })();
    // Reap on every path; user code cannot keep a hung interpreter alive.
    let _ = child.kill();
    let _ = child.wait();
    result
}
use std::io::Read;
