//! Live RF/IF element catalogues, never options inferred from a model name.
use crate::{Error, Result, Session, pna};
#[derive(Clone, Debug)]
pub struct Element { pub name: String, pub values: Vec<String>, pub current: String }
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub resource: String, pub channel: u32, pub idn: String,
    pub configurations: Vec<String>, pub name: String, pub description: String,
    pub elements: Vec<Element>,
}
pub fn inspect(s: &mut dyn Session, c: &rf_core::Config) -> Result<Snapshot> {
    c.instrument.validate().map_err(Error::Protocol)?;
    let idn=s.query("*IDN?")?;
    if !pna::is_pna(&idn) || !c.instrument.expected_idn.is_empty()
        && !idn.to_lowercase().contains(&c.instrument.expected_idn.to_lowercase())
    { return Err(Error::Protocol("Identité incompatible avec le Path Configurator".into())); }
    let ch=c.instrument.channel;
    let names=pna::list(&s.query(&format!("SENS{ch}:PATH:CONF:ELEM:CAT?"))?);
    if names.len()>128 || names.iter().any(|n|!rf_core::instrument::scpi_name(n)) {
        return Err(Error::Protocol("Catalogue de chemins invalide".into()));
    }
    let mut elements=Vec::new();
    for name in names {
        let values=pna::list(&s.query(&format!("SENS{ch}:PATH:CONF:ELEM:VAL:CAT? \"{name}\""))?);
        let current=s.query(&format!("SENS{ch}:PATH:CONF:ELEM? \"{name}\""))?.trim().trim_matches('"').to_owned();
        if values.is_empty() || values.len()>128 || values.iter().any(|v|!rf_core::instrument::scpi_name(v))
            || !values.contains(&current) { return Err(Error::Protocol(format!("État de {name} non reconnu"))); }
        elements.push(Element { name, values, current });
    }
    Ok(Snapshot { resource:c.resource.clone(),channel:ch,idn,
        configurations:pna::list(&s.query("SENS:PATH:CONF:CAT?")?),
        name:s.query(&format!("SENS{ch}:PATH:CONF:NAME?"))?.trim().trim_matches('"').into(),
        description:s.query(&format!("SENS{ch}:PATH:CONF:DTEX?"))?.trim().trim_matches('"').into(),elements })
}
/// Recall first: it may replace IF settings. The acquisition driver configures
/// sweep, IFBW, power and calibration afterwards, and does not trigger here.
pub fn apply(s: &mut dyn Session, c: &rf_core::Config) -> Result<()> {
    let path=&c.instrument.pna.path;
    if !path.apply { return Ok(()); }
    if !path.validate() { return Err(Error::Protocol("Réglages de chemin invalides".into())); }
    let actual=inspect(s,c)?;
    if !path.configuration.is_empty() && !actual.configurations.contains(&path.configuration) {
        return Err(Error::Protocol("Configuration de chemin absente de l'appareil".into()));
    }
    for (name,value) in &path.elements {
        if !actual.elements.iter().any(|e| &e.name==name && e.values.contains(value)) {
            return Err(Error::Protocol(format!("{name} / {value} indisponible sur cet appareil")));
        }
    }
    let ch=c.instrument.channel;
    s.write(&format!("SENS{ch}:SWE:MODE HOLD"))?;
    if !path.configuration.is_empty() { s.write(&format!("SENS{ch}:PATH:CONF:SEL \"{}\"",path.configuration))?; }
    for (name,value) in &path.elements {
        s.write(&format!("SENS{ch}:PATH:CONF:ELEM:SEL \"{name}\",\"{value}\""))?;
        let confirmed=s.query(&format!("SENS{ch}:PATH:CONF:ELEM? \"{name}\""))?;
        if confirmed.trim().trim_matches('"')!=value { return Err(Error::Protocol(format!("Chemin {name} non confirmé"))); }
    }
    pna::check_error(s,"Configuration RF/IF")
}
