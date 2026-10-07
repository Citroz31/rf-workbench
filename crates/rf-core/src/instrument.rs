//! Persisted instrument controls; detected capabilities are deliberately transient.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PnaSetup {
    pub reference_ohm: f64,
    pub power_sweep: bool,
    pub power_start_dbm: f64,
    pub power_stop_dbm: f64,
    pub source_port: u8,
    pub all_s_parameters: bool,
    pub calset: String,
    pub apply_calset: bool,
    pub result_path: String,
    pub fixture: Fixture,
}
impl Default for PnaSetup {
    fn default() -> Self {
        Self {
            reference_ohm: 50.,
            power_sweep: false,
            power_start_dbm: -30.,
            power_stop_dbm: -10.,
            source_port: 1,
            all_s_parameters: false,
            calset: String::new(),
            apply_calset: false,
            result_path: String::new(),
            fixture: Fixture::default(),
        }
    }
}
impl PnaSetup {
    pub fn validate(&self) -> Result<(), String> {
        if !self.reference_ohm.is_finite()
            || !(0.1..=10000.).contains(&self.reference_ohm)
            || [self.power_start_dbm, self.power_stop_dbm]
                .iter()
                .any(|v| !v.is_finite() || !(-160. ..=30.).contains(v))
            || self.power_stop_dbm <= self.power_start_dbm
            || !(1..=4).contains(&self.source_port)
            || !scpi_name(&self.calset)
            || self.result_path.len() > 4096
            || self.fixture.input_s2p.len() + self.fixture.output_s2p.len() > 3_500_000
        {
            return Err("Configuration PNA / fixture invalide".into());
        }
        Ok(())
    }
}
pub fn scpi_name(s: &str) -> bool {
    s.len() <= 256 && !s.contains(['\n', '\r', '\0', '"', '\'', ';'])
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Fixture {
    pub enabled: bool,
    pub input_s2p: String,
    pub output_s2p: String,
    pub reverse_input: bool,
    pub reverse_output: bool,
    pub provenance: String,
}
impl Default for Fixture {
    fn default() -> Self {
        Self {
            enabled: false,
            input_s2p: String::new(),
            output_s2p: String::new(),
            reverse_input: false,
            reverse_output: true,
            provenance: String::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Controls {
    pub pna: PnaSetup,
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
            pna: PnaSetup::default(),
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
        self.pna.validate()?;
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
