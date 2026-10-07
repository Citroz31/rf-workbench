//! Network framing: big-endian u32 length followed by a serde JSON IqFrame.
//! UDP uses one complete packet and is capped at 60000 bytes.
use crate::{Result, local_capabilities};
use rf_core::dsp::{IqFrame, Settings};
use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs, UdpSocket},
    time::{Duration, Instant},
};
fn read_deadline(s: &mut TcpStream, mut bytes: &mut [u8], deadline: Instant) -> Result<()> {
    while !bytes.is_empty() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("Lecture TCP I/Q expirée".into());
        }
        s.set_read_timeout(Some(remaining))
            .map_err(|e| e.to_string())?;
        let n = s.read(bytes).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("TCP fermé avant la fin de la trame".into());
        }
        bytes = &mut bytes[n..];
    }
    Ok(())
}
fn write_deadline(s: &mut TcpStream, mut bytes: &[u8], deadline: Instant) -> Result<()> {
    while !bytes.is_empty() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("Écriture TCP I/Q expirée".into());
        }
        s.set_write_timeout(Some(remaining))
            .map_err(|e| e.to_string())?;
        let n = s.write(bytes).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("Écriture TCP sans progrès".into());
        }
        bytes = &bytes[n..];
    }
    Ok(())
}
pub enum Io {
    RawReader(crate::recording::Reader),
    RawWriter(crate::recording::Writer),
    Tcp(TcpStream, Duration),
    Udp(UdpSocket),
    Visa(crate::Instrument, Box<Settings>),
    Plugin(crate::plugin::Plugin),
}
impl Io {
    pub fn open(c: &Settings, source: bool) -> Result<Self> {
        c.validate()?;
        if !matches!(
            c.io_backend.as_str(),
            "SOAPY" | "UHD" | "IIO" | "ZMQ" | "AUDIO" | "HDF5" | "PARQUET"
        ) {
            local_capabilities().check(c)?;
        }
        let timeout = Duration::from_millis(c.timeout_ms);
        match c.io_backend.as_str() {
            "RAW" => {
                if source {
                    Ok(Self::RawReader(crate::recording::Reader::open(
                        std::path::Path::new(&c.endpoint),
                    )?))
                } else {
                    Ok(Self::RawWriter(crate::recording::Writer::create(
                        std::path::Path::new(&c.endpoint),
                    )?))
                }
            }
            "TCP" => {
                let deadline = Instant::now() + timeout;
                let mut last = "Aucune adresse".to_string();
                for addr in c.endpoint.to_socket_addrs().map_err(|e| e.to_string())? {
                    match TcpStream::connect_timeout(
                        &addr,
                        deadline.saturating_duration_since(Instant::now()),
                    ) {
                        Ok(s) => {
                            s.set_read_timeout(Some(timeout))
                                .map_err(|e| e.to_string())?;
                            s.set_write_timeout(Some(timeout))
                                .map_err(|e| e.to_string())?;
                            s.set_nodelay(true).map_err(|e| e.to_string())?;
                            return Ok(Self::Tcp(s, timeout));
                        }
                        Err(e) => last = e.to_string(),
                    }
                }
                Err(last)
            }
            "UDP" => {
                let s = if source {
                    UdpSocket::bind(&c.endpoint)
                } else {
                    UdpSocket::bind("0.0.0.0:0")
                }
                .map_err(|e| e.to_string())?;
                s.set_read_timeout(Some(timeout))
                    .map_err(|e| e.to_string())?;
                s.set_write_timeout(Some(timeout))
                    .map_err(|e| e.to_string())?;
                if !source {
                    s.connect(&c.endpoint).map_err(|e| e.to_string())?
                }
                Ok(Self::Udp(s))
            }
            "VISA" | "SERIAL" => {
                if !source {
                    return Err("Écriture I/Q SCPI : commande propre au modèle requise".into());
                }
                let p = crate::InstrumentProfile {
                    name: "Capture I/Q SCPI".into(),
                    class: crate::InstrumentClass::Sdr,
                    resource: c.endpoint.clone(),
                    capabilities: local_capabilities(),
                    query: c.scpi_query.clone(),
                    format: c.format.clone(),
                    timeout_ms: c.timeout_ms,
                    reconnects: c.reconnects,
                };
                Ok(Self::Visa(crate::Instrument::open(p)?, Box::new(c.clone())))
            }
            "SOAPY" | "UHD" | "IIO" | "ZMQ" | "AUDIO" | "HDF5" | "PARQUET" => {
                Ok(Self::Plugin(crate::plugin::Plugin::open(c, source)?))
            }
            _ => Err("Backend HAL inconnu".into()),
        }
    }
    pub fn read(&mut self) -> Result<IqFrame> {
        let f = match self {
            Self::RawReader(r) => r.read_frame()?,
            Self::Tcp(s, timeout) => {
                let deadline = Instant::now() + *timeout;
                let mut h = [0; 4];
                read_deadline(s, &mut h, deadline)?;
                let n = u32::from_be_bytes(h) as usize;
                if n > 4 * 1024 * 1024 || n == 0 {
                    return Err("Paquet I/Q trop volumineux".into());
                }
                let mut b = vec![0; n];
                read_deadline(s, &mut b, deadline)?;
                serde_json::from_slice(&b).map_err(|e| e.to_string())?
            }
            Self::Udp(s) => {
                let mut b = vec![0; 65536];
                let n = s.recv(&mut b).map_err(|e| e.to_string())?;
                if n > 60000 {
                    return Err("Datagramme I/Q trop volumineux".into());
                }
                serde_json::from_slice(&b[..n]).map_err(|e| e.to_string())?
            }
            Self::Visa(i, c) => {
                if c.format != "cf32_le" {
                    return Err("Capture VISA : sélectionner cf32_le après configuration du format instrument".into());
                }
                let bytes = i.binary()?;
                IqFrame {
                    samples: crate::recording::decode_cf32(&bytes)?,
                    sample_rate: c.rate,
                    center_hz: c.center_hz,
                    unit: rf_core::dsp::Unit::Fs,
                    simulated: false,
                    time: rf_core::dsp::TimeTag {
                        clock_domain: "instrument-unspecified".into(),
                        discontinuity: true,
                        ..Default::default()
                    },
                }
            }
            Self::Plugin(p) => p.read()?,
            _ => return Err("HAL ouverte en écriture".into()),
        };
        f.validate()?;
        Ok(f)
    }
    pub fn write(&mut self, f: &IqFrame) -> Result<()> {
        f.validate()?;
        match self {
            Self::RawWriter(w) => w.append(f),
            Self::Tcp(s, timeout) => {
                let deadline = Instant::now() + *timeout;
                let b = serde_json::to_vec(f).map_err(|e| e.to_string())?;
                if b.len() > 4 * 1024 * 1024 {
                    return Err("Paquet trop volumineux".into());
                }
                write_deadline(s, &(b.len() as u32).to_be_bytes(), deadline)?;
                write_deadline(s, &b, deadline)
            }
            Self::Udp(s) => {
                let b = serde_json::to_vec(f).map_err(|e| e.to_string())?;
                if b.len() > 60000 {
                    return Err(
                        "UDP : réduire le nombre d'échantillons (paquet <= 60000 octets)".into(),
                    );
                }
                s.send(&b).map(|_| ()).map_err(|e| e.to_string())
            }
            Self::Plugin(p) => p.write(f),
            _ => Err("HAL ouverte en lecture".into()),
        }
    }
}
