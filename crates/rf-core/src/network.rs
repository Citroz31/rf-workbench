//! Complex network data retains all four two-port terms for fixture removal.
use crate::dsp::Complex;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TwoPort {
    pub frequency_hz: Vec<f64>,
    /// Touchstone order: S11, S21, S12, S22.
    pub s: Vec<[Complex; 4]>,
    pub z0: f64,
    pub simulated: bool,
}
impl TwoPort {
    pub fn validate(&self) -> Result<(), String> {
        if !(2..=crate::MAX_POINTS).contains(&self.s.len())
            || self.frequency_hz.len() != self.s.len()
            || !self.z0.is_finite()
            || !(0.1..=10000.).contains(&self.z0)
            || self.frequency_hz.iter().any(|f| !f.is_finite() || *f < 0.)
            || self.frequency_hz.windows(2).any(|p| p[1] <= p[0])
            || self
                .s
                .iter()
                .flatten()
                .any(|z| !z.re.is_finite() || !z.im.is_finite() || z.norm2() > 1e20)
        {
            return Err("Réseau complexe 2 ports invalide".into());
        }
        Ok(())
    }
    pub fn reverse(&mut self) {
        for s in &mut self.s {
            *s = [s[3], s[2], s[1], s[0]];
        }
    }
    pub fn trace(&self, parameter: &str) -> Result<crate::NetworkTrace, String> {
        let index = match parameter {
            "S11" => 0,
            "S21" => 1,
            "S12" => 2,
            "S22" => 3,
            _ => return Err("Paramètre absent du réseau 2 ports".into()),
        };
        Ok(crate::NetworkTrace {
            frequency_hz: self.frequency_hz.clone(),
            magnitude_db: self
                .s
                .iter()
                .map(|s| 10. * s[index].norm2().max(1e-30).log10())
                .collect(),
            phase_deg: self.s.iter().map(|s| s[index].arg().to_degrees()).collect(),
            parameter: parameter.into(),
            simulated: self.simulated,
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Curve {
    pub node: u64,
    pub name: String,
    pub x: Vec<f64>,
    pub x_unit: String,
    pub y: Vec<f64>,
    pub y_unit: String,
    pub phase_deg: Option<Vec<f64>>,
    pub simulated: bool,
    pub corrected: bool,
}
impl Curve {
    pub fn from_trace(node: u64, t: &crate::NetworkTrace) -> Self {
        Self {
            node,
            name: t.parameter.clone(),
            x: t.frequency_hz.clone(),
            x_unit: "Hz".into(),
            y: t.magnitude_db.clone(),
            y_unit: "dB".into(),
            phase_deg: Some(t.phase_deg.clone()),
            simulated: t.simulated,
            corrected: false,
        }
    }
}
