//! Persisted instrument controls; detected capabilities are deliberately transient.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Controls {
    /// Displayed physical RF ports; independent from a PNA measurement channel.
    pub port_count: u8,
    pub dc: DcControls,
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
            port_count: 2,
            dc: DcControls::default(),
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
        if !(1..=4).contains(&self.port_count)
            || self.expected_idn.len() > 256
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

/// Setpoints are persisted; enabling a physical supply is a separate explicit
/// worker command. Graph loading and Run never arm a DC output.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DcControls {
    pub channel: u8,
    pub voltage_v: f64,
    pub current_limit_a: f64,
}
impl Default for DcControls {
    fn default() -> Self {
        Self {
            channel: 1,
            voltage_v: 0.,
            current_limit_a: 0.1,
        }
    }
}
impl DcControls {
    pub fn validate(&self, kind: crate::Kind) -> Result<(), String> {
        let (low, high, current) = match (kind, self.channel) {
            (crate::Kind::DcSupplyE3631A, 1) => (0., 6., 5.),
            (crate::Kind::DcSupplyE3631A, 2) => (0., 25., 1.),
            (crate::Kind::DcSupplyE3631A, 3) => (-25., 0., 1.),
            (crate::Kind::DcSupplyE36313A, 1) => (0., 6., 10.),
            (crate::Kind::DcSupplyE36313A, 2 | 3) => (0., 25., 2.),
            _ => return Err("Modèle ou canal DC invalide".into()),
        };
        if !self.voltage_v.is_finite()
            || !(low..=high).contains(&self.voltage_v)
            || !self.current_limit_a.is_finite()
            || !(0. ..=current).contains(&self.current_limit_a)
        {
            return Err("Consignes DC hors plage nominale du canal".into());
        }
        Ok(())
    }
}
