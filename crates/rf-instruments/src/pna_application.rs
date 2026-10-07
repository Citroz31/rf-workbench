//! Formatted data from existing licensed PNA application channels.
use crate::{Error, Result, Session, pna};
use rf_core::Config;
#[derive(Clone, Debug)]
pub struct ApplicationData {
    pub channel: u32,
    pub class: String,
    pub measurement: String,
    pub parameter: String,
    pub display_format: String,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
}
fn selected(s: &mut dyn Session, c: &Config) -> Result<(String, String, String)> {
    c.instrument.validate().map_err(Error::Protocol)?;
    let idn = s.query("*IDN?")?;
    if !pna::is_pna(&idn)
        || (!c.instrument.expected_idn.trim().is_empty()
            && !idn
                .to_lowercase()
                .contains(&c.instrument.expected_idn.trim().to_lowercase()))
    {
        return Err(Error::Protocol("PNA non identifié".into()));
    }
    let ch = c.instrument.channel;
    let class = s
        .query(&format!("SENS{ch}:CLAS:NAME?"))?
        .trim_matches(['"', '\''])
        .to_owned();
    let classes = pna::list(&s.query("SYST:MCL:VAL:CAT?")?);
    if !classes.contains(&class) {
        return Err(Error::Protocol(
            "Classe du canal non autorisée ou indéterminée".into(),
        ));
    }
    let measurements = pna::measurements(&s.query(&format!("CALC{ch}:PAR:CAT:EXT?"))?)?;
    let name = &c.instrument.measurement;
    let (name, param) = measurements
        .iter()
        .find(|(n, _)| n == name)
        .ok_or_else(|| Error::Protocol("Choisir le nom d'une mesure détectée du canal".into()))?;
    if name.contains(['"', '\'', '\n', '\r', ';', '\0']) {
        return Err(Error::Protocol("Nom de mesure invalide".into()));
    }
    s.write(&format!("CALC{ch}:PAR:SEL \"{name}\",FAST"))?;
    Ok((class, name.clone(), param.clone()))
}
pub fn acquire(s: &mut dyn Session, c: &Config) -> Result<ApplicationData> {
    let (class, measurement, parameter) = selected(s, c)?;
    let ch = c.instrument.channel;
    let display_format = s.query(&format!("CALC{ch}:FORM?"))?;
    let (old, border) = pna::saved_format(s)?;
    let result = (|| {
        s.write("FORM:BORD SWAP")?;
        s.write("FORM:DATA REAL,64")?;
        let x = pna::decode(&s.read_binary(&format!("CALC{ch}:X?"))?, 64)?;
        s.write(&format!("FORM:DATA REAL,{}", c.instrument.precision))?;
        let y = pna::decode(
            &s.read_binary(&format!("CALC{ch}:DATA? FDATA"))?,
            c.instrument.precision,
        )?;
        if x.len() != y.len() {
            return Err(Error::Protocol(
                "FDATA : format scalaire requis (pas Polar / Smith)".into(),
            ));
        }
        Ok(ApplicationData {
            channel: ch,
            class,
            measurement,
            parameter,
            display_format,
            x,
            y,
        })
    })();
    pna::finish_transfer(s, &old, &border, result)
}
pub fn set_compression(s: &mut dyn Session, c: &Config) -> Result<()> {
    let (class, _, _) = selected(s, c)?;
    if class != "Gain Compression" {
        return Err(Error::Protocol(
            "Réglage GCA exige un canal Gain Compression autorisé".into(),
        ));
    }
    s.write(&format!(
        "SENS{}:GCS:COMP:LEV {}",
        c.instrument.channel, c.instrument.compression_db
    ))?;
    let value = s
        .query(&format!("SENS{}:GCS:COMP:LEV?", c.instrument.channel))?
        .trim()
        .parse::<f64>()
        .map_err(|e| Error::Protocol(e.to_string()))?;
    if !value.is_finite() || (value - c.instrument.compression_db).abs() > 1e-6 {
        return Err(Error::Protocol(
            "Niveau GCA non confirmé par l'appareil".into(),
        ));
    }
    Ok(())
}
