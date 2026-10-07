//! Versioned DUT catalogue. Importers will supply sourced, reviewed manufacturer data.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Source {
    pub url: String,
    pub retrieved_on: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Specification {
    pub typical: f64,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RfCharacteristics {
    pub band_hz: [f64; 2],
    pub reference_frequency_hz: f64,
    pub gain_db: Specification,
    pub noise_figure_db: Specification,
    pub output_p1db_dbm: Specification,
    pub conditions: String,
    pub revision: String,
    pub modes: Vec<String>,
    pub attenuation_states: u8,
    pub attenuation_step_db: f64,
    pub phase_states: u8,
    pub phase_step_deg: f64,
    pub notes: Vec<String>,
}
impl RfCharacteristics {
    fn valid(&self) -> bool {
        self.band_hz.iter().all(|v| v.is_finite() && *v > 0.)
            && self.band_hz[0] < self.band_hz[1]
            && self.reference_frequency_hz.is_finite()
            && (self.band_hz[0]..=self.band_hz[1]).contains(&self.reference_frequency_hz)
            && [&self.gain_db, &self.noise_figure_db, &self.output_p1db_dbm]
                .iter()
                .all(|s| {
                    s.typical.is_finite()
                        && (-160. ..=160.).contains(&s.typical)
                        && s.minimum.is_none_or(|v| v.is_finite() && v <= s.typical)
                        && s.maximum.is_none_or(|v| v.is_finite() && v >= s.typical)
                })
            && self.noise_figure_db.typical >= 0.
            && self.attenuation_states > 0
            && self.attenuation_states <= 64
            && self.phase_states > 0
            && self.phase_states <= 64
            && self.attenuation_step_db.is_finite()
            && (0. ..=160.).contains(&self.attenuation_step_db)
            && self.phase_step_deg.is_finite()
            && (0. ..=360.).contains(&self.phase_step_deg)
            && (self.attenuation_states - 1) as f64 * self.attenuation_step_db <= 160.
            && (self.phase_states - 1) as f64 * self.phase_step_deg < 360.
            && !self.conditions.is_empty()
            && !self.revision.is_empty()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Component {
    pub id: String,
    pub manufacturer: String,
    pub part_number: String,
    pub category: String,
    pub description: String,
    pub insertion_loss_db: Option<f64>,
    pub noise_figure_db: Option<f64>,
    #[serde(default)]
    pub rf: Option<RfCharacteristics>,
    #[serde(default)]
    pub sources: Vec<Source>,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Catalog {
    pub schema_version: u32,
    pub manufacturers: Vec<String>,
    pub entries: Vec<Component>,
}
impl Default for Catalog {
    fn default() -> Self {
        Self::from_json(include_str!("../data/catalog.json")).expect("bundled catalogue")
    }
}
impl Catalog {
    pub fn from_json(text: &str) -> Result<Self, Error> {
        if text.len() > 4_000_000 {
            return Err(Error::Invalid("Catalogue trop volumineux".into()));
        }
        let catalog: Self = serde_json::from_str(text)?;
        catalog.validate()?;
        Ok(catalog)
    }
    pub fn validate(&self) -> Result<(), Error> {
        if self.schema_version != 1 || self.entries.len() > 10_000 {
            return Err(Error::Invalid(
                "Version ou taille de catalogue invalide".into(),
            ));
        }
        let mut ids = BTreeSet::new();
        for c in &self.entries {
            if c.rf
                .as_ref()
                .is_some_and(|rf| !rf.valid() || c.sources.is_empty())
            {
                return Err(Error::Invalid(format!(
                    "Caractéristiques RF invalides ou non sourcées : {}",
                    c.id
                )));
            }
            if c.noise_figure_db.is_some_and(|v| v > 60.) {
                return Err(Error::Invalid("Facteur de bruit hors domaine".into()));
            }
            if c.id.trim().is_empty()
                || c.manufacturer.trim().is_empty()
                || c.part_number.trim().is_empty()
                || !ids.insert(&c.id)
                || [c.insertion_loss_db, c.noise_figure_db]
                    .into_iter()
                    .flatten()
                    .any(|v| !v.is_finite() || !(0. ..=160.).contains(&v))
                || c.sources
                    .iter()
                    .any(|s| !s.url.starts_with("https://") || s.retrieved_on.trim().is_empty())
            {
                return Err(Error::Invalid(format!("Fiche DUT invalide : {}", c.id)));
            }
        }
        Ok(())
    }
    pub fn search<'a>(&'a self, query: &str) -> Vec<&'a Component> {
        let q = query.to_lowercase();
        self.entries
            .iter()
            .filter(|c| {
                format!(
                    "{} {} {} {}",
                    c.manufacturer, c.part_number, c.category, c.description
                )
                .to_lowercase()
                .contains(&q)
            })
            .collect()
    }
    pub fn to_json(&self) -> Result<String, Error> {
        self.validate()?;
        Ok(serde_json::to_string_pretty(self)?)
    }
    pub fn include_bundled(&mut self) {
        for entry in Self::default().entries {
            if !self.entries.iter().any(|e| e.id == entry.id) {
                self.entries.push(entry);
            }
        }
        if !self.manufacturers.iter().any(|s| s == "MACOM") {
            self.manufacturers.push("MACOM".into());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn component() -> Component {
        Component {
            id: "test-dut".into(),
            manufacturer: "Lab".into(),
            part_number: "TEST-1".into(),
            category: "Atténuateur".into(),
            description: "Fiche locale de test".into(),
            insertion_loss_db: Some(3.),
            noise_figure_db: None,
            rf: None,
            sources: vec![],
            attributes: BTreeMap::new(),
        }
    }
    #[test]
    fn bundled_manufacturer_specs_have_sources_and_conditions() {
        let c = Catalog::default();
        c.validate().unwrap();
        assert_eq!(c.entries.len(), 2);
        let lna = c.search("MAAL-FR1245")[0].rf.as_ref().unwrap();
        assert_eq!(
            (
                lna.gain_db.typical,
                lna.noise_figure_db.typical,
                lna.output_p1db_dbm.typical
            ),
            (26., 1.2, 5.)
        );
        let core = c.search("CGY2170")[0].rf.as_ref().unwrap();
        assert_eq!((core.attenuation_states, core.phase_states), (64, 64));
        assert_eq!(core.attenuation_step_db * 63., 31.5);
        assert_eq!(core.phase_step_deg * 63., 354.375);
        assert!(!core.notes.is_empty());
    }
    #[test]
    fn legacy_and_user_catalogs_keep_custom_entries_when_seeded() {
        let json = serde_json::to_string(&component()).unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value.as_object_mut().unwrap().remove("rf");
        assert!(
            serde_json::from_value::<Component>(value)
                .unwrap()
                .rf
                .is_none()
        );
        let mut c = Catalog {
            schema_version: 1,
            manufacturers: vec![],
            entries: vec![component()],
        };
        c.include_bundled();
        c.include_bundled();
        assert_eq!(c.entries.len(), 3);
    }
    #[test]
    fn round_trip_and_search() {
        let mut c = Catalog::default();
        c.entries.push(component());
        let loaded = Catalog::from_json(&c.to_json().unwrap()).unwrap();
        assert_eq!(loaded.search("test-1").len(), 1);
        assert!(loaded.search("unknown").is_empty());
    }
    #[test]
    fn duplicate_bad_units_and_unsourced_urls_rejected() {
        let mut c = Catalog {
            entries: vec![component(), component()],
            ..Default::default()
        };
        assert!(c.validate().is_err());
        c.entries.pop();
        c.entries[0].insertion_loss_db = Some(f64::NAN);
        assert!(c.validate().is_err());
        c.entries[0].insertion_loss_db = Some(3.);
        c.entries[0].sources.push(Source {
            url: "http://invalid".into(),
            retrieved_on: "2026-10-07".into(),
        });
        assert!(c.validate().is_err());
    }
}
