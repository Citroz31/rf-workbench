//! VISA-style resource manager: simulator, native SCPI TCP and vendor VISA.
pub mod discovery;
pub mod pna;
pub mod pna_application;
pub mod visa;
use rf_core::{Config, Trace};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
use thiserror::Error;

const MAX_RESPONSE: usize = 8 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Protocol(String),
    #[error(
        "Ressource non prise en charge : {0}. Utiliser SIM::RF::INSTR, TCPIP::hôte::port::SOCKET ou une ressource VISA."
    )]
    Unsupported(String),
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resource {
    Sim,
    Tcp { host: String, port: u16 },
    Visa(String),
}
impl Resource {
    pub fn parse(value: &str) -> Result<Self> {
        if value == "SIM::RF::INSTR" {
            return Ok(Self::Sim);
        }
        let p: Vec<_> = value.split("::").collect();
        if p.len() == 4 && (p[0] == "TCPIP" || p[0] == "TCPIP0") && p[3] == "SOCKET" {
            let port = p[2]
                .parse::<u16>()
                .ok()
                .filter(|v| *v > 0)
                .ok_or_else(|| Error::Protocol("Port TCP invalide".into()))?;
            if p[1].is_empty() || p[1].len() > 253 || p[1].chars().any(char::is_whitespace) {
                return Err(Error::Protocol("Hôte TCP invalide".into()));
            }
            return Ok(Self::Tcp {
                host: p[1].into(),
                port,
            });
        }
        if !value.contains(['\n', '\r', '\0'])
            && value.len() <= 1024
            && ((p.len() >= 3
                && p.last() == Some(&"INSTR")
                && ["USB", "GPIB", "TCPIP"]
                    .iter()
                    .any(|prefix| p[0].starts_with(prefix)))
                || (p.len() == 2 && p[0].starts_with("ASRL") && p[1] == "INSTR"))
        {
            return Ok(Self::Visa(value.into()));
        }
        Err(Error::Unsupported(value.into()))
    }
}

pub trait Session: Send {
    fn write(&mut self, command: &str) -> Result<()>;
    fn query(&mut self, command: &str) -> Result<String>;
    fn read_binary(&mut self, command: &str) -> Result<Vec<u8>>;
}

#[derive(Default)]
pub struct ResourceManager;
impl ResourceManager {
    pub fn list_resources(&self) -> Vec<String> {
        vec!["SIM::RF::INSTR".into()]
    }
    pub fn open_resource(&self, name: &str, timeout: Duration) -> Result<Box<dyn Session>> {
        match Resource::parse(name)? {
            Resource::Sim => Ok(Box::new(SimSession::default())),
            Resource::Visa(name) => Ok(Box::new(visa::VisaSession::open(&name, timeout)?)),
            Resource::Tcp { host, port } => {
                // OS DNS lookup can outlive timeout; all calls are on a worker.
                let addresses = (host.as_str(), port).to_socket_addrs()?;
                let deadline = std::time::Instant::now() + timeout;
                let mut last_error = None;
                for address in addresses {
                    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                    if remaining.is_zero() {
                        break;
                    }
                    match TcpStream::connect_timeout(&address, remaining) {
                        Ok(stream) => {
                            stream.set_read_timeout(Some(timeout))?;
                            stream.set_write_timeout(Some(timeout))?;
                            stream.set_nodelay(true)?;
                            return Ok(Box::new(TcpSession {
                                stream: BufReader::new(stream),
                                timeout,
                                poisoned: false,
                            }));
                        }
                        Err(e) => last_error = Some(e),
                    }
                }
                Err(Error::Io(last_error.unwrap_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::TimedOut, "Connexion TCP expirée")
                })))
            }
        }
    }
}

fn validate_command(command: &str) -> Result<()> {
    if command.is_empty() || command.len() > 65_536 || command.contains(['\n', '\r', '\0']) {
        return Err(Error::Protocol(
            "Commande SCPI vide, trop longue ou contenant un séparateur".into(),
        ));
    }
    Ok(())
}

pub struct TcpSession {
    stream: BufReader<TcpStream>,
    timeout: Duration,
    poisoned: bool,
}
struct DeadlineReader<'a> {
    reader: &'a mut BufReader<TcpStream>,
    deadline: std::time::Instant,
}
impl Read for DeadlineReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let remaining = self
            .deadline
            .saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Lecture SCPI expirée",
            ));
        }
        self.reader.get_ref().set_read_timeout(Some(remaining))?;
        self.reader.read(buffer)
    }
}
impl TcpSession {
    fn text_response(&mut self) -> Result<String> {
        let deadline = std::time::Instant::now() + self.timeout;
        let mut bytes = Vec::new();
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(Error::Protocol("Réponse SCPI expirée".into()));
            }
            self.stream.get_ref().set_read_timeout(Some(remaining))?;
            let available = self.stream.fill_buf()?;
            if available.is_empty() {
                return Err(Error::Protocol(
                    "Connexion fermée avant la fin de la réponse".into(),
                ));
            }
            let count = available
                .iter()
                .position(|b| *b == b'\n')
                .map_or(available.len(), |i| i + 1);
            let ended = available[count - 1] == b'\n';
            if bytes.len() + count > MAX_RESPONSE {
                return Err(Error::Protocol("Réponse SCPI trop volumineuse".into()));
            }
            bytes.extend_from_slice(&available[..count]);
            self.stream.consume(count);
            if ended {
                break;
            }
        }
        String::from_utf8(bytes)
            .map(|s| s.trim_end_matches(['\r', '\n']).to_owned())
            .map_err(|_| Error::Protocol("Réponse non UTF-8 ; utiliser le lecteur binaire".into()))
    }
}
impl Session for TcpSession {
    fn write(&mut self, command: &str) -> Result<()> {
        validate_command(command)?;
        if self.poisoned {
            return Err(Error::Protocol(
                "Session désynchronisée ; reconnecter l'instrument".into(),
            ));
        }
        self.stream.get_ref().set_read_timeout(Some(self.timeout))?;
        self.stream.get_mut().write_all(command.as_bytes())?;
        self.stream.get_mut().write_all(b"\n")?;
        self.stream.get_mut().flush()?;
        Ok(())
    }
    fn query(&mut self, command: &str) -> Result<String> {
        self.write(command)?;
        let result = self.text_response();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn read_binary(&mut self, command: &str) -> Result<Vec<u8>> {
        self.write(command)?;
        let mut reader = DeadlineReader {
            reader: &mut self.stream,
            deadline: std::time::Instant::now() + self.timeout,
        };
        let result = read_definite_block(&mut reader);
        // A SCPI binary response must end with LF or CRLF in this backend.
        let result = result.and_then(|bytes| {
            let mut end = [0];
            reader.read_exact(&mut end)?;
            if end[0] == b'\r' {
                reader.read_exact(&mut end)?;
            }
            if end[0] != b'\n' {
                return Err(Error::Protocol("Terminaison binaire SCPI invalide".into()));
            }
            Ok(bytes)
        });
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
}

/// IEEE 488.2 definite-length block: #<digit count><length><payload>.
pub fn read_definite_block(reader: &mut impl Read) -> Result<Vec<u8>> {
    let mut header = [0u8; 2];
    reader.read_exact(&mut header)?;
    if header[0] != b'#' || !(b'1'..=b'9').contains(&header[1]) {
        return Err(Error::Protocol(
            "Bloc IEEE 488.2 invalide (longueur indéfinie non prise en charge)".into(),
        ));
    }
    let mut length = vec![0u8; (header[1] - b'0') as usize];
    reader.read_exact(&mut length)?;
    if !length.iter().all(u8::is_ascii_digit) {
        return Err(Error::Protocol("Longueur binaire non numérique".into()));
    }
    let len = std::str::from_utf8(&length)
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n <= MAX_RESPONSE)
        .ok_or_else(|| Error::Protocol("Bloc binaire trop volumineux".into()))?;
    let mut payload = vec![0; len];
    reader.read_exact(&mut payload)?;
    Ok(payload)
}

#[derive(Debug)]
pub struct SimSession {
    frequency: f64,
    power: f64,
    enabled: bool,
}
impl Default for SimSession {
    fn default() -> Self {
        Self {
            frequency: 2.45e9,
            power: -10.,
            enabled: false,
        }
    }
}
impl Session for SimSession {
    fn write(&mut self, command: &str) -> Result<()> {
        validate_command(command)?;
        let upper = command.trim().to_ascii_uppercase();
        if upper == "*RST" {
            *self = Self::default();
            return Ok(());
        }
        for (prefix, value) in [(":FREQ ", &mut self.frequency), (":POW ", &mut self.power)] {
            if let Some(raw) = upper.strip_prefix(prefix) {
                let number = raw
                    .parse::<f64>()
                    .map_err(|_| Error::Protocol("Valeur SCPI invalide".into()))?;
                if !number.is_finite() {
                    return Err(Error::Protocol("Valeur SCPI non finie".into()));
                }
                *value = number;
                return Ok(());
            }
        }
        match upper.as_str() {
            ":OUTP ON" | ":OUTP 1" => self.enabled = true,
            ":OUTP OFF" | ":OUTP 0" => self.enabled = false,
            _ => return Err(Error::Protocol(format!("Commande non simulée : {command}"))),
        }
        Ok(())
    }
    fn query(&mut self, command: &str) -> Result<String> {
        validate_command(command)?;
        match command.trim().to_ascii_uppercase().as_str() {
            "*IDN?" => Ok("RF Workbench,RF Simulator,SIM-001,0.1".into()),
            "*OPC?" => Ok("1".into()),
            ":FREQ?" => Ok(self.frequency.to_string()),
            ":POW?" => Ok(self.power.to_string()),
            ":OUTP?" => Ok(u8::from(self.enabled).to_string()),
            ":SYST:ERR?" | "SYST:ERR?" => Ok("0,\"No error\"".into()),
            _ => Err(Error::Protocol(format!("Requête non simulée : {command}"))),
        }
    }
    fn read_binary(&mut self, _: &str) -> Result<Vec<u8>> {
        Err(Error::Unsupported("Simulation de transfert binaire".into()))
    }
}

pub fn simulate_trace(
    c: &Config,
    tone_hz: f64,
    level_dbm: f64,
    sequence: u64,
) -> rf_core::Result<Trace> {
    if !(2..=rf_core::MAX_POINTS).contains(&c.points)
        || !c.start_hz.is_finite()
        || !c.stop_hz.is_finite()
        || c.stop_hz <= c.start_hz
        || !tone_hz.is_finite()
        || !level_dbm.is_finite()
    {
        return Err(rf_core::Error::Invalid("Balayage simulé invalide".into()));
    }
    let mut frequency_hz = Vec::with_capacity(c.points);
    let mut amplitude_dbm = Vec::with_capacity(c.points);
    let span = c.stop_hz - c.start_hz;
    for i in 0..c.points {
        let f = c.start_hz + span * i as f64 / (c.points - 1) as f64;
        let carrier = level_dbm - ((f - tone_hz) / (span * 0.025)).powi(2) * 12.;
        let spur = level_dbm - 32. - ((f - tone_hz - span * 0.22) / (span * 0.018)).powi(2) * 12.;
        let floor = -94.
            + ((i as f64 * 1.79 + sequence as f64 * 0.17).sin() * 2.8)
            + (i as f64 * 0.71).cos() * 1.5;
        frequency_hz.push(f);
        amplitude_dbm.push(carrier.max(spur).max(floor));
    }
    let trace = Trace {
        frequency_hz,
        amplitude_dbm,
        simulated: true,
    };
    trace.validate()?;
    Ok(trace)
}

pub fn self_tests() -> Vec<rf_core::TestResult> {
    let mut sim = SimSession::default();
    vec![
        rf_core::TestResult {
            name: "Session SCPI simulée".into(),
            passed: sim.write(":FREQ 2400000000").is_ok()
                && sim.query(":FREQ?").ok().as_deref() == Some("2400000000"),
            detail: "write / query et état de l'instrument".into(),
        },
        rf_core::TestResult {
            name: "Bloc binaire IEEE 488.2".into(),
            passed: read_definite_block(&mut &b"#14test"[..]).ok().as_deref() == Some(b"test")
                && read_definite_block(&mut &b"#0bad"[..]).is_err(),
            detail: "Longueur définie décodée, en-tête invalide refusé".into(),
        },
        rf_core::TestResult {
            name: "Séparation SOCKET / VISA INSTR".into(),
            passed: matches!(
                Resource::parse("TCPIP0::localhost::INSTR"),
                Ok(Resource::Visa(_))
            ),
            detail: "VXI-11 est délégué au runtime VISA, jamais à un socket brut".into(),
        },
        rf_core::TestResult {
            name: "Pic du banc simulé".into(),
            passed: simulate_trace(&Config::default(), 2.45e9, -13., 0)
                .and_then(|t| t.peak())
                .is_ok_and(|p| p == (2.45e9, -13.)),
            detail: "Perte DUT de 3 dB, pic attendu à -13 dBm".into(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integrated_suite_passes() {
        for r in self_tests() {
            assert!(r.passed, "{}", r.name);
        }
    }
    #[test]
    fn resources_are_strict() {
        assert_eq!(
            Resource::parse("TCPIP0::127.0.0.1::5025::SOCKET").unwrap(),
            Resource::Tcp {
                host: "127.0.0.1".into(),
                port: 5025
            }
        );
        for s in [
            "unknown::0::INSTR",
            "TCPIP::x::0::SOCKET",
            "TCPIP::::5025::SOCKET",
        ] {
            assert!(Resource::parse(s).is_err());
        }
    }
    #[test]
    fn truncated_and_oversized_binary_rejected() {
        for s in [
            b"#14abc".as_slice(),
            b"#19x".as_slice(),
            b"#899999999".as_slice(),
        ] {
            assert!(read_definite_block(&mut &s[..]).is_err());
        }
    }
    #[test]
    fn newline_injection_rejected() {
        assert!(SimSession::default().write(":POW 0\n:OUTP ON").is_err());
    }
    #[test]
    fn tcp_queries_keep_fragmented_responses_separate() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut s = BufReader::new(stream);
            let mut command = String::new();
            s.read_line(&mut command).unwrap();
            assert_eq!(command, "*IDN?\n");
            s.get_mut().write_all(b"ACME,").unwrap();
            s.get_mut().write_all(b"SA,001,1\r\n").unwrap();
            command.clear();
            s.read_line(&mut command).unwrap();
            assert_eq!(command, "*OPC?\n");
            s.get_mut().write_all(b"1\n").unwrap();
        });
        let mut s = ResourceManager
            .open_resource(
                &format!("TCPIP::127.0.0.1::{port}::SOCKET"),
                Duration::from_secs(2),
            )
            .unwrap();
        assert_eq!(s.query("*IDN?").unwrap(), "ACME,SA,001,1");
        assert_eq!(s.query("*OPC?").unwrap(), "1");
        server.join().unwrap();
    }
    #[test]
    fn tcp_binary_then_text_is_not_desynchronized() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (s, _) = listener.accept().unwrap();
            let mut s = BufReader::new(s);
            let mut line = String::new();
            s.read_line(&mut line).unwrap();
            s.get_mut().write_all(b"#14test\n").unwrap();
            line.clear();
            s.read_line(&mut line).unwrap();
            s.get_mut().write_all(b"1\n").unwrap();
        });
        let mut s = ResourceManager
            .open_resource(
                &format!("TCPIP::127.0.0.1::{port}::SOCKET"),
                Duration::from_secs(2),
            )
            .unwrap();
        assert_eq!(s.read_binary("DATA?").unwrap(), b"test");
        assert_eq!(s.query("*OPC?").unwrap(), "1");
        server.join().unwrap();
    }
    #[test]
    fn tcp_timeout_poisons_session() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (_s, _) = listener.accept().unwrap();
            std::thread::sleep(Duration::from_millis(160));
        });
        let mut s = ResourceManager
            .open_resource(
                &format!("TCPIP::127.0.0.1::{port}::SOCKET"),
                Duration::from_millis(40),
            )
            .unwrap();
        assert!(s.query("*IDN?").is_err());
        assert!(s.query("*OPC?").is_err());
        server.join().unwrap();
    }
}
