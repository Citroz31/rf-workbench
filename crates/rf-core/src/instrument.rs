//! Persisted instrument controls; detected capabilities are deliberately transient.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Controls {
    pub expected_idn: String,
    pub timeout_ms: u64,
    pub channel: u32,
    pub measurement: String,
    pub precision: u8,
    pub configure_sweep: bool,
    pub trigger: bool,
    pub if_bandwidth_hz: f64,
    pub averaging: bool,
    pub averages: u32,
    pub compression_db: f64,
    pub read_query: String,
    pub set_command: String,
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            expected_idn: String::new(),
            timeout_ms: 5000,
            channel: 1,
            measurement: String::new(),
            precision: 32,
            configure_sweep: false,
            trigger: false,
            if_bandwidth_hz: 1000.,
            averaging: false,
            averages: 4,
            compression_db: 1.,
            read_query: "READ?".into(),
            set_command: String::new(),
        }
    }
}
impl Controls {
    pub fn validate(&self) -> Result<(), String> {
        if self.expected_idn.len() > 256
            || !(1..=30000).contains(&self.timeout_ms)
            || !(1..=1000).contains(&self.channel)
            || !matches!(self.precision, 32 | 64)
            || self.measurement.len() > 256
            || self
                .measurement
                .contains(['\n', '\r', '\0', '"', '\'', ';'])
            || !self.if_bandwidth_hz.is_finite()
            || self.if_bandwidth_hz <= 0.
            || self.if_bandwidth_hz > 1e8
            || !(1..=65536).contains(&self.averages)
            || !self.compression_db.is_finite()
            || !(0.01..=100.).contains(&self.compression_db)
            || [&self.read_query, &self.set_command]
                .iter()
                .any(|s| s.len() > 1024 || s.contains(['\n', '\r', '\0']))
        {
            return Err("Réglages instrument invalides (canal, délai, format ou commande)".into());
        }
        Ok(())
    }
}
