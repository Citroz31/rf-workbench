//! Shared acquisition contracts: explicit capabilities, recorded provenance,
//! bounded transfers and worker-owned instrument sessions.
pub mod plugin;
pub mod queue;
pub mod recording;
pub mod streaming;
pub mod transport;
use rf_core::dsp::{IqFrame, Settings};
use rf_instruments::{ResourceManager, Session};
use serde::{Deserialize, Serialize};
use std::time::Duration;
pub type Result<T> = std::result::Result<T, String>;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstrumentClass {
    Generator,
    SpectrumAnalyzer,
    Vna,
    Oscilloscope,
    PowerMeter,
    Sdr,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Capabilities {
    pub clocks: Vec<String>,
    pub triggers: Vec<String>,
    pub hardware_timestamps: bool,
    pub ptp: bool,
    pub gpsdo: bool,
    pub max_channels: usize,
}
impl Capabilities {
    pub fn check(&self, c: &Settings) -> Result<()> {
        if !self.clocks.iter().any(|x| x == &c.clock)
            || !self.triggers.iter().any(|x| x == &c.trigger)
            || c.mimo_channels > self.max_channels
            || (c.clock == "PTP" && !self.ptp)
            || (c.clock == "GPSDO" && !self.gpsdo)
        {
            return Err("Horloge, trigger ou MIMO non déclaré par ce pilote/profil".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstrumentProfile {
    pub name: String,
    pub class: InstrumentClass,
    pub resource: String,
    pub capabilities: Capabilities,
    pub query: String,
    pub format: String,
    pub timeout_ms: u64,
    pub reconnects: usize,
}
impl InstrumentProfile {
    pub fn validate(&self) -> Result<()> {
        rf_instruments::Resource::parse(&self.resource).map_err(|e| e.to_string())?;
        if self.name.len() > 256
            || self.query.len() > 1024
            || !self.query.trim_end().ends_with('?')
            || self.query.contains(['\r', '\n', '\0', ';'])
            || !(1..=30000).contains(&self.timeout_ms)
            || self.reconnects > 5
        {
            return Err("Profil instrument invalide".into());
        }
        Ok(())
    }
}
/// Reconnection is available only for the explicit read-only identification
/// query. Acquisition queries and configuration writes are never replayed.
pub struct Instrument {
    profile: InstrumentProfile,
    session: Box<dyn Session>,
}
impl Instrument {
    fn require_class(&self, class: InstrumentClass) -> Result<()> {
        if self.profile.class != class {
            return Err("Opération incompatible avec la classe du profil".into());
        }
        Ok(())
    }
    /// Generic SCPI dialect; the profile/model must validate these commands.
    pub fn set_generator(&mut self, hz: f64, dbm: f64) -> Result<()> {
        self.require_class(InstrumentClass::Generator)?;
        if !hz.is_finite() || hz <= 0. || !dbm.is_finite() || !(-160. ..=30.).contains(&dbm) {
            return Err("Consigne RF invalide".into());
        }
        self.write(&format!(":FREQ {hz}"))?;
        self.write(&format!(":POW {dbm}"))
    }
    fn ascii(&mut self) -> Result<Vec<f64>> {
        let query = self.profile.query.clone();
        let text = self.query(&query)?;
        let values: Vec<f64> = text
            .split([',', ';', ' ', '\t'])
            .filter(|s| !s.is_empty())
            .map(|s| s.parse::<f64>().map_err(|e| e.to_string()))
            .collect::<Result<_>>()?;
        if values.is_empty() || values.len() > 131072 || values.iter().any(|x| !x.is_finite()) {
            return Err("Acquisition ASCII vide/non finie/trop longue".into());
        }
        Ok(values)
    }
    pub fn read_power(&mut self, unit: rf_core::dsp::Unit) -> Result<rf_core::dsp::Data> {
        self.require_class(InstrumentClass::PowerMeter)?;
        if !matches!(unit, rf_core::dsp::Unit::Dbm | rf_core::dsp::Unit::Watt) {
            return Err("Unité power meter invalide".into());
        }
        let values = self.ascii()?;
        if values.len() != 1 {
            return Err("Power meter : une valeur attendue".into());
        }
        Ok(rf_core::dsp::Data::Quantity {
            value: values[0],
            unit,
            simulated: self.profile.resource == "SIM::RF::INSTR",
        })
    }
    pub fn read_spectrum(
        &mut self,
        frequency_hz: Vec<f64>,
        unit: rf_core::dsp::Unit,
    ) -> Result<rf_core::dsp::Spectrum> {
        self.require_class(InstrumentClass::SpectrumAnalyzer)?;
        let s = rf_core::dsp::Spectrum {
            frequency_hz,
            levels: self.ascii()?,
            unit,
            simulated: self.profile.resource == "SIM::RF::INSTR",
        };
        rf_core::dsp::Data::Spectrum(s.clone()).validate()?;
        Ok(s)
    }
    pub fn read_vna(
        &mut self,
        frequency_hz: Vec<f64>,
        parameter: &str,
    ) -> Result<rf_core::NetworkTrace> {
        self.require_class(InstrumentClass::Vna)?;
        if !matches!(parameter, "S11" | "S21" | "S12" | "S22") {
            return Err("Paramètre S invalide".into());
        }
        let data = self.ascii()?;
        if data.len() != frequency_hz.len() * 2
            || frequency_hz.len() < 2
            || frequency_hz.windows(2).any(|w| w[1] <= w[0])
            || frequency_hz.iter().any(|f| !f.is_finite() || *f <= 0.)
        {
            return Err("VNA : grille et paires réelles/imaginaires incompatibles".into());
        }
        let values: Vec<_> = data
            .chunks_exact(2)
            .map(|z| rf_core::dsp::Complex::new(z[0], z[1]))
            .collect();
        let magnitude_db: Vec<_> = values.iter().map(|z| 10. * z.norm2().log10()).collect();
        if magnitude_db.iter().any(|x| !x.is_finite()) {
            return Err("VNA : module nul/non fini".into());
        }
        Ok(rf_core::NetworkTrace {
            frequency_hz,
            magnitude_db,
            phase_deg: values.iter().map(|z| z.arg().to_degrees()).collect(),
            parameter: parameter.into(),
            simulated: self.profile.resource == "SIM::RF::INSTR",
        })
    }
    pub fn read_oscilloscope(&mut self, rate: f64, unit: rf_core::dsp::Unit) -> Result<IqFrame> {
        self.require_class(InstrumentClass::Oscilloscope)?;
        let f = IqFrame {
            samples: self
                .ascii()?
                .into_iter()
                .map(|x| rf_core::dsp::Complex::new(x, 0.))
                .collect(),
            sample_rate: rate,
            center_hz: 0.,
            unit,
            simulated: self.profile.resource == "SIM::RF::INSTR",
            time: rf_core::dsp::TimeTag {
                clock_domain: "instrument-unspecified".into(),
                discontinuity: true,
                ..Default::default()
            },
        };
        f.validate()?;
        Ok(f)
    }
    pub fn open(profile: InstrumentProfile) -> Result<Self> {
        profile.validate()?;
        let session = ResourceManager
            .open_resource(&profile.resource, Duration::from_millis(profile.timeout_ms))
            .map_err(|e| e.to_string())?;
        Ok(Self { profile, session })
    }
    pub fn identify(&mut self) -> Result<String> {
        let mut last = String::new();
        for attempt in 0..=self.profile.reconnects {
            match self.session.query("*IDN?") {
                Ok(s) => return Ok(s),
                Err(e) => last = e.to_string(),
            }
            if attempt < self.profile.reconnects {
                self.session = ResourceManager
                    .open_resource(
                        &self.profile.resource,
                        Duration::from_millis(self.profile.timeout_ms),
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        Err(last)
    }
    pub fn write(&mut self, command: &str) -> Result<()> {
        self.session.write(command).map_err(|e| e.to_string())
    }
    pub fn query(&mut self, command: &str) -> Result<String> {
        self.session.query(command).map_err(|e| e.to_string())
    }
    pub fn binary(&mut self) -> Result<Vec<u8>> {
        self.session
            .read_binary(&self.profile.query)
            .map_err(|e| e.to_string())
    }
}
pub fn discover_visa() -> Result<Vec<String>> {
    rf_instruments::visa::discover("?*INSTR").map_err(|e| e.to_string())
}
/// Adapter boundary for SDR drivers and optional storage backends. Drivers
/// must report capabilities before any requested synchronization is accepted.
pub trait Acquisition: Send {
    fn capabilities(&self) -> &Capabilities;
    fn read(&mut self) -> Result<IqFrame>;
    fn write(&mut self, frame: &IqFrame) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
}
pub fn local_capabilities() -> Capabilities {
    Capabilities {
        clocks: vec!["internal".into()],
        triggers: vec!["immediate".into()],
        max_channels: 1,
        ..Default::default()
    }
}
