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
pub struct Component {
    pub id: String,
    pub manufacturer: String,
    pub part_number: String,
    pub category: String,
    pub description: String,
    pub insertion_loss_db: Option<f64>,
    pub noise_figure_db: Option<f64>,
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
            sources: vec![],
            attributes: BTreeMap::new(),
        }
    }
    #[test]
    fn empty_seed_contains_no_invented_chip_specs() {
        assert!(Catalog::default().entries.is_empty());
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
