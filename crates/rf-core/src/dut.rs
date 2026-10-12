//! Editable scalar DUT model. Catalogue values seed a project-owned copy.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProductType {
    #[default]
    Generic,
    Lna,
    Pa,
    Corechip,
    Passive,
}
impl ProductType {
    pub const ALL: [Self; 5] = [Self::Generic, Self::Lna, Self::Pa, Self::Corechip, Self::Passive];
    pub fn label(self) -> &'static str {
        match self {
            Self::Generic => "DUT générique",
            Self::Lna => "LNA · faible bruit",
            Self::Pa => "PA · amplificateur de puissance",
            Self::Corechip => "Corechip · atténuation / phase",
            Self::Passive => "Passif · pertes / paramètres S",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Model {
    pub product_type: ProductType,
    pub s11_db: f64,
    pub s22_db: f64,
    pub s12_db: f64,
    pub output_ip3_dbm: Option<f64>,
    pub saturation_dbm: Option<f64>,
    pub attenuation_states: u8,
    pub attenuation_step_db: f64,
    pub phase_states: u8,
    pub phase_step_deg: f64,
}
impl Default for Model {
    fn default() -> Self {
        Self { product_type: ProductType::Generic, s11_db: -20., s22_db: -20., s12_db: -60., output_ip3_dbm: None, saturation_dbm: None, attenuation_states: 64, attenuation_step_db: 0.5, phase_states: 64, phase_step_deg: 5.625 }
    }
}
impl Model {
    pub fn valid(&self) -> bool {
        [self.s11_db, self.s22_db, self.s12_db].iter().all(|v| v.is_finite() && (-300. ..=60.).contains(v))
            && [self.output_ip3_dbm, self.saturation_dbm].iter().all(|v| v.is_none_or(|v| v.is_finite() && (-160. ..=100.).contains(&v)))
            && (1..=64).contains(&self.attenuation_states) && (1..=64).contains(&self.phase_states)
            && self.attenuation_step_db.is_finite() && (0. ..=160.).contains(&self.attenuation_step_db)
            && self.phase_step_deg.is_finite() && (0. ..=360.).contains(&self.phase_step_deg)
            && f64::from(self.attenuation_states - 1) * self.attenuation_step_db <= 160.
            && f64::from(self.phase_states - 1) * self.phase_step_deg < 360.
    }
}
