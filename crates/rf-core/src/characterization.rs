//! Sourced scalar curves. Missing phase stays missing; no impedance is inferred.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    pub url: String,
    pub page: u32,
    pub revision: String,
    pub conditions: String,
    pub method: String,
    pub approximate: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Characteristic {
    pub parameter: String,
    pub frequency_hz: Vec<f64>,
    pub values: Vec<f64>,
    pub unit: String,
    #[serde(default)]
    pub phase_deg: Option<Vec<f64>>,
    pub provenance: Provenance,
}
impl Characteristic {
    pub fn validate(&self) -> Result<(), String> {
        if !["S11", "S12", "S21", "S22", "NF", "OP1dB", "OIP3"].contains(&self.parameter.as_str())
            || self.unit
                != if matches!(self.parameter.as_str(), "OP1dB" | "OIP3") {
                    "dBm"
                } else {
                    "dB"
                }
            || !(2..=10001).contains(&self.values.len())
            || self.frequency_hz.len() != self.values.len()
            || self.frequency_hz.iter().any(|f| !f.is_finite() || *f <= 0.)
            || self.frequency_hz.windows(2).any(|w| w[1] <= w[0])
            || self
                .values
                .iter()
                .any(|v| !v.is_finite() || !(-300. ..=160.).contains(v))
            || self.parameter == "NF" && self.values.iter().any(|v| *v < 0.)
            || self.phase_deg.as_ref().is_some_and(|p| {
                p.len() != self.values.len()
                    || p.iter().any(|v| !v.is_finite() || v.abs() > 1e6)
                    || !self.parameter.starts_with('S')
            })
            || !(self.provenance.url.starts_with("https://")
                || self.provenance.url.starts_with("file://"))
            || self.provenance.url.len() > 2048
            || self.provenance.url.starts_with("https://") && self.provenance.page == 0
            || self.provenance.conditions.trim().is_empty()
            || self.provenance.method.trim().is_empty()
            || self.provenance.revision.trim().is_empty()
        {
            return Err(format!(
                "Courbe de caractérisation invalide : {}",
                self.parameter
            ));
        }
        Ok(())
    }
    /// Piecewise linear interpolation in the published unit. Never extrapolate.
    pub fn sample(&self, frequency: f64) -> Result<(f64, Option<f64>), String> {
        let x = &self.frequency_hz;
        if !frequency.is_finite() || x.len() < 2 || frequency < x[0] || frequency > x[x.len() - 1] {
            return Err(format!(
                "{} : fréquence hors des données publiées",
                self.parameter
            ));
        }
        let hi = x.partition_point(|f| *f < frequency).clamp(1, x.len() - 1);
        let lo = hi - 1;
        let w = (frequency - x[lo]) / (x[hi] - x[lo]);
        let interpolate = |values: &[f64]| values[lo] + w * (values[hi] - values[lo]);
        // Phase samples are unwrapped locally before interpolation.
        let phase = self.phase_deg.as_ref().map(|p| {
            let delta = (p[hi] - p[lo] + 180.).rem_euclid(360.) - 180.;
            p[lo] + w * delta
        });
        Ok((interpolate(&self.values), phase))
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Dataset {
    pub label: String,
    pub curves: Vec<Characteristic>,
    #[serde(default)]
    pub reference_ohm: Option<f64>,
}
impl Dataset {
    pub fn validate(&self) -> Result<(), String> {
        let mut names = std::collections::BTreeSet::new();
        if self.label.trim().is_empty()
            || self.label.len() > 512
            || self.curves.is_empty()
            || self.curves.len() > 16
            || self
                .reference_ohm
                .is_some_and(|z| !z.is_finite() || !(0.1..=10000.).contains(&z))
            || self.reference_ohm.is_none() && self.curves.iter().any(|c| c.phase_deg.is_some())
        {
            return Err("Jeu de caractérisation invalide".into());
        }
        for c in &self.curves {
            c.validate()?;
            if !names.insert(&c.parameter) {
                return Err("Paramètre répété dans le jeu de données".into());
            }
        }
        Ok(())
    }
    pub fn curve(&self, parameter: &str) -> Option<&Characteristic> {
        self.curves.iter().find(|c| c.parameter == parameter)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interpolation_keeps_unknown_phase_and_refuses_extrapolation() {
        let c = Characteristic {
            parameter: "S21".into(),
            frequency_hz: vec![1e9, 2e9],
            values: vec![10., 20.],
            unit: "dB".into(),
            phase_deg: None,
            provenance: Provenance {
                url: "https://example.com/test.pdf".into(),
                page: 1,
                revision: "test".into(),
                conditions: "test".into(),
                method: "test fixture".into(),
                approximate: true,
            },
        };
        c.validate().unwrap();
        assert_eq!(c.sample(1.5e9).unwrap(), (15., None));
        assert!(c.sample(0.9e9).is_err());
        assert_eq!(c.sample(2e9).unwrap().0, 20.);
    }
}
