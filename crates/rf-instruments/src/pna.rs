//! Keysight PNA / PNA-X binary acquisition. Sweep/calibration writes are opt-in.
//! SCPI reference: helpfiles.keysight.com/csg/NA520xA/Programming/GP-IB_Command_Finder/
use crate::{Error, ResourceManager, Result, Session};
use rf_core::{Config, Kind, MAX_POINTS, NetworkTrace};
use std::time::Duration;

fn configure_power_and_cal(s: &mut dyn Session, c: &Config) -> Result<()> {
    let ch = c.instrument.channel;
    let setup = &c.instrument.pna;
    if setup.source_port > c.instrument.port_count {
        return Err(Error::Protocol("Port source hors configuration".into()));
    }
    if !c.power_dbm.is_finite() || !(-160. ..=30.).contains(&c.power_dbm) {
        return Err(Error::Protocol("Puissance source invalide".into()));
    }
    let requested = if setup.power_sweep {
        [setup.power_start_dbm, setup.power_stop_dbm]
    } else {
        [c.power_dbm, c.power_dbm]
    };
    let ports: Vec<_> = if setup.power_sweep {
        vec![setup.source_port]
    } else {
        (1..=c.instrument.port_count).collect()
    };
    for port in ports {
        let min = number(&s.query(&format!("SOUR{ch}:POW{port}? MIN"))?)?;
        let max = number(&s.query(&format!("SOUR{ch}:POW{port}? MAX"))?)?;
        if requested[0] < min || requested[1] > max {
            return Err(Error::Protocol(format!(
                "Puissance hors plage rapportée port {port}: {min}–{max} dBm"
            )));
        }
    }
    if setup.apply_calset {
        if setup.calset.trim().is_empty() {
            return Err(Error::Protocol(
                "Choisir un CalSet avant de l'appliquer".into(),
            ));
        }
        let catalog = list(&s.query("CSET:CAT? NAME")?);
        if !catalog.contains(&setup.calset) {
            return Err(Error::Protocol("CalSet absent de l'appareil".into()));
        }
        s.write(&format!("SENS{ch}:CORR:CSET:ACT \"{}\",0", setup.calset))?;
        s.write(&format!("SENS{ch}:CORR:STAT ON"))?;
    }
    if !setup.power_sweep {
        s.write(&format!("SOUR{ch}:POW:COUP ON"))?;
        s.write(&format!("SOUR{ch}:POW {}", c.power_dbm))?;
    }
    Ok(())
}
fn number(text: &str) -> Result<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && v.abs() < 1e30)
        .ok_or_else(|| Error::Protocol("Valeur numérique instrument invalide".into()))
}
/// Explicitly add standard S traces to an existing Standard channel.
pub fn prepare(s: &mut dyn Session, c: &Config) -> Result<String> {
    c.instrument.validate().map_err(Error::Protocol)?;
    let ch = c.instrument.channel;
    if !is_pna(&s.query("*IDN?")?) {
        return Err(Error::Protocol("PNA/PNA-X requis".into()));
    }
    let ports = number(&s.query("SYST:CAP:HARD:PORT:COUN?")?)? as u8;
    if c.instrument.port_count > ports {
        return Err(Error::Protocol(
            "Ports demandés absents de l'appareil".into(),
        ));
    }
    if s.query(&format!("SENS{ch}:CLAS:NAME?"))?
        .trim()
        .trim_matches('"')
        != "Standard"
    {
        return Err(Error::Protocol("Canal Standard existant requis".into()));
    }
    let catalog = measurements(&s.query(&format!("CALC{ch}:PAR:CAT:EXT?"))?)?;
    let mut count = 0;
    for i in 1..=c.instrument.port_count {
        for j in 1..=c.instrument.port_count {
            let p = format!("S{i}{j}");
            if !catalog.iter().any(|(_, param)| param == &p) {
                let name = format!("RFW_CH{ch}_{p}");
                if catalog.iter().any(|(n, _)| n == &name) {
                    return Err(Error::Protocol(
                        "Nom RFW existant avec un autre paramètre".into(),
                    ));
                }
                s.write(&format!("CALC{ch}:PAR:DEF:EXT \"{name}\",\"{p}\""))?;
                count += 1;
            }
        }
    }
    let error = s.query("SYST:ERR?")?;
    if !error.trim_start().starts_with("0,") {
        return Err(Error::Protocol(format!("Préparation PNA : {error}")));
    }
    Ok(format!(
        "{count} traces ajoutées au canal {ch}. Relire ses capacités ; aucun sweep lancé."
    ))
}
pub fn acquire_curves(
    s: &mut dyn Session,
    c: &Config,
    kind: Kind,
    node: u64,
) -> Result<Vec<rf_core::network::Curve>> {
    if c.instrument.pna.power_sweep {
        return power_curves(s, c, kind, node);
    }
    let mut curves = Vec::new();
    let mut first = c.clone();
    if c.instrument.pna.all_s_parameters {
        let ch = c.instrument.channel;
        if !c.instrument.trigger
            && s.query(&format!("SENS{ch}:SWE:MODE?"))?
                .trim()
                .trim_matches('"')
                != "HOLD"
        {
            return Err(Error::Protocol(
                "Matrice multi-traces : canal HOLD requis ou activer Déclencher (sweep unique)"
                    .into(),
            ));
        }
        let catalog = measurements(&s.query(&format!("CALC{ch}:PAR:CAT:EXT?"))?)?;
        // Resolve every trace before any sweep write, avoiding partial matrices.
        let mut selected = Vec::new();
        for i in 1..=c.instrument.port_count {
            for j in 1..=c.instrument.port_count {
                let p = format!("S{i}{j}");
                let names: Vec<_> = catalog.iter().filter(|(_, param)| param == &p).collect();
                let name = if p == c.s_parameter && !c.instrument.measurement.is_empty() {
                    names
                        .iter()
                        .find(|(n, _)| *n == c.instrument.measurement)
                        .copied()
                } else if names.len() == 1 {
                    Some(names[0])
                } else {
                    None
                }
                .ok_or_else(|| {
                    Error::Protocol(format!(
                        "{p}: créer une trace unique dans ce canal avant de lire la matrice"
                    ))
                })?;
                selected.push((p, name.0.clone()));
            }
        }
        // Measure the selected trace first to configure/trigger exactly once.
        selected.sort_by_key(|(p, _)| p != &c.s_parameter);
        for (index, (p, name)) in selected.into_iter().enumerate() {
            first.s_parameter = p;
            first.instrument.measurement = name;
            if index > 0 {
                first.instrument.configure_sweep = false;
                first.instrument.trigger = false;
            }
            let t = acquire_vna(s, &first, kind)?;
            curves.push(rf_core::network::Curve::from_trace(node, &t));
        }
    } else {
        let t = acquire_vna(s, c, kind)?;
        curves.push(rf_core::network::Curve::from_trace(node, &t));
    }
    Ok(curves)
}
fn power_curves(
    s: &mut dyn Session,
    c: &Config,
    kind: Kind,
    node: u64,
) -> Result<Vec<rf_core::network::Curve>> {
    c.instrument.validate().map_err(Error::Protocol)?;
    let ch = c.instrument.channel;
    let p = &c.instrument.pna;
    if c.instrument.port_count > kind.max_rf_ports().unwrap_or(0)
        || p.source_port > c.instrument.port_count
        || !(2..=MAX_POINTS).contains(&c.points)
    {
        return Err(Error::Protocol("Ports/points invalides".into()));
    }
    let id = s.query("*IDN?")?;
    if !(is_pna(&id) || kind == Kind::UsbVna && is_usb_vna(&id))
        || !c.instrument.expected_idn.is_empty()
            && !id
                .to_lowercase()
                .contains(&c.instrument.expected_idn.to_lowercase())
    {
        return Err(Error::Protocol("Identité instrument différente".into()));
    }
    if number(&s.query("SYST:CAP:HARD:PORT:COUN?")?)? < f64::from(c.instrument.port_count) {
        return Err(Error::Protocol("Ports physiques insuffisants".into()));
    }
    if s.query(&format!("SENS{ch}:CLAS:NAME?"))?
        .trim()
        .trim_matches('"')
        != "Standard"
    {
        return Err(Error::Protocol(
            "Sweep puissance Standard requis ; GCA reste une application distincte".into(),
        ));
    }
    let (_, source) = rf_core::physical::s_parameter_ports(&c.s_parameter)
        .ok_or_else(|| Error::Protocol("Paramètre S invalide".into()))?;
    if source != p.source_port {
        return Err(Error::Protocol(
            "Le port source du paramètre S doit correspondre au sweep puissance".into(),
        ));
    }
    let cat = measurements(&s.query(&format!("CALC{ch}:PAR:CAT:EXT?"))?)?;
    let entries: Vec<_> = cat
        .iter()
        .filter(|(name, param)| {
            param == &c.s_parameter
                && (c.instrument.measurement.is_empty() || name == &c.instrument.measurement)
        })
        .collect();
    if entries.len() != 1 {
        return Err(Error::Protocol(
            "Choisir une seule trace S existante".into(),
        ));
    }
    s.write(&format!("CALC{ch}:PAR:SEL \"{}\",FAST", entries[0].0))?;
    if c.instrument.configure_sweep {
        let min = number(&s.query("SYST:CAP:FREQ:MIN?")?)?;
        let max = number(&s.query("SYST:CAP:FREQ:MAX?")?)?;
        if !c.frequency_hz.is_finite() || c.frequency_hz < min || c.frequency_hz > max {
            return Err(Error::Protocol("Fréquence CW hors plage".into()));
        }
        configure_power_and_cal(s, c)?;
        s.write(&format!("SENS{ch}:SWE:TYPE POW"))?;
        s.write(&format!("SENS{ch}:FREQ:CW {}", c.frequency_hz))?;
        s.write(&format!("SOUR{ch}:POW:COUP OFF"))?;
        s.write(&format!(
            "SOUR{ch}:POW{}:PORT:STAR {}",
            p.source_port, p.power_start_dbm
        ))?;
        s.write(&format!(
            "SOUR{ch}:POW{}:PORT:STOP {}",
            p.source_port, p.power_stop_dbm
        ))?;
        s.write(&format!("SENS{ch}:SWE:POIN {}", c.points))?;
        s.write(&format!("SENS{ch}:BWID {}", c.instrument.if_bandwidth_hz))?;
        s.write(&format!("SENS{ch}:AVER:COUN {}", c.instrument.averages))?;
        s.write(&format!(
            "SENS{ch}:AVER {}",
            if c.instrument.averaging { "ON" } else { "OFF" }
        ))?;
    } else if !s
        .query(&format!("SENS{ch}:SWE:TYPE?"))?
        .trim()
        .starts_with("POW")
    {
        return Err(Error::Protocol(
            "Le canal n'est pas en sweep de puissance".into(),
        ));
    }
    if c.instrument.trigger {
        s.write(&format!("SENS{ch}:SWE:MODE HOLD"))?;
        s.write(&format!("INIT{ch}:IMM"))?;
        if s.query("*OPC?")?.trim() != "1" {
            return Err(Error::Protocol("Sweep non terminé".into()));
        }
    }
    let (old_data, old_order) = saved_format(s)?;
    let result = (|| {
        s.write("FORM:BORD SWAP")?;
        s.write("FORM:DATA REAL,64")?;
        let x = decode(&s.read_binary(&format!("CALC{ch}:X?"))?, 64)?;
        s.write(&format!("FORM:DATA REAL,{}", c.instrument.precision))?;
        let data = decode(
            &s.read_binary(&format!("CALC{ch}:DATA? SDATA"))?,
            c.instrument.precision,
        )?;
        if !(2..=MAX_POINTS).contains(&x.len())
            || data.len() != 2 * x.len()
            || x.windows(2).any(|p| p[1] <= p[0])
            || x.iter().any(|p| !(-200. ..=100.).contains(p))
        {
            return Err(Error::Protocol("Axe puissance/SDATA incohérent".into()));
        }
        let y = data
            .chunks_exact(2)
            .map(|z| 10. * (z[0] * z[0] + z[1] * z[1]).max(1e-30).log10())
            .collect();
        let phase = data
            .chunks_exact(2)
            .map(|z| z[1].atan2(z[0]).to_degrees())
            .collect();
        Ok(vec![rf_core::network::Curve {
            node,
            name: c.s_parameter.clone(),
            x,
            x_unit: "dBm".into(),
            y,
            y_unit: "dB".into(),
            phase_deg: Some(phase),
            simulated: false,
            corrected: false,
        }])
    })();
    finish_transfer(s, &old_data, &old_order, result)
}

#[derive(Clone, Debug, Default)]
pub struct Capabilities {
    pub port_count: Option<u8>,
    pub resource: String,
    pub idn: String,
    pub options: String,
    pub licenses: String,
    pub valid_classes: Option<Vec<String>>,
    pub channels: Vec<u32>,
    pub channel: u32,
    pub class: String,
    pub measurements: Vec<(String, String)>,
    pub frequency_range: Option<(f64, f64)>,
    pub warnings: Vec<String>,
}
impl Capabilities {
    /// Never infer licenses from a model number or the general class catalog.
    pub fn supports(&self, class: &str) -> bool {
        self.valid_classes
            .as_ref()
            .is_some_and(|v| v.iter().any(|x| x.eq_ignore_ascii_case(class)))
    }
}
pub fn list(text: &str) -> Vec<String> {
    text.trim()
        .trim_matches(['"', '\''])
        .split(',')
        .map(|x| x.trim().trim_matches(['"', '\'']).to_owned())
        .filter(|x| !x.is_empty())
        .collect()
}
/// CSV including individually quoted names and embedded commas.
pub fn measurements(text: &str) -> Result<Vec<(String, String)>> {
    let text = text.trim();
    let text = if text.starts_with('"') && text.ends_with('"') && text.matches('"').count() == 2 {
        &text[1..text.len() - 1]
    } else {
        text
    };
    let mut fields = Vec::new();
    let mut value = String::new();
    let mut quote = None;
    for c in text.trim().chars() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, ',') => {
                fields.push(value.trim().to_owned());
                value.clear();
            }
            _ => value.push(c),
        }
    }
    if quote.is_some() {
        return Err(Error::Protocol("Catalogue de mesures mal fermé".into()));
    }
    fields.push(value.trim().to_owned());
    if fields == ["NO CATALOG"] || fields == [""] {
        return Ok(Vec::new());
    }
    if fields.len() % 2 != 0 || fields.len() > 4096 {
        return Err(Error::Protocol("Catalogue de mesures invalide".into()));
    }
    Ok(fields
        .chunks_exact(2)
        .map(|p| (p[0].clone(), p[1].clone()))
        .collect())
}
fn query(resource: &str, timeout: Duration, command: &str) -> Result<String> {
    // An unsupported query may poison a session. Each optional capability gets
    // its own bounded session, never reusing a connection after a timeout.
    ResourceManager
        .open_resource(resource, timeout)?
        .query(command)
}
pub fn inspect(resource: &str, channel: u32, timeout: Duration) -> Result<Capabilities> {
    if !(1..=1000).contains(&channel) {
        return Err(Error::Protocol("Canal invalide".into()));
    }
    let idn = query(resource, timeout, "*IDN?")?;
    if !is_pna(&idn) {
        return Err(Error::Protocol(
            "Le profil PNA exige un Keysight/Agilent N52xx/E83xx identifié".into(),
        ));
    }
    let mut c = Capabilities {
        resource: resource.into(),
        idn,
        channel,
        ..Default::default()
    };
    match query(resource, timeout, "SYST:CAP:HARD:PORT:COUN?") {
        Ok(s) => c.port_count = s.trim().parse::<u8>().ok().filter(|n| *n > 0),
        Err(e) => c.warnings.push(e.to_string()),
    }
    for (command, target) in [
        ("*OPT?", 0),
        ("SYST:CAP:LIC:CAT? VALID", 1),
        ("SYST:MCL:VAL:CAT?", 2),
        ("SYST:CHAN:CAT?", 3),
    ] {
        match query(resource, timeout, command) {
            Ok(s) => match target {
                0 => c.options = s,
                1 => c.licenses = s,
                2 => c.valid_classes = Some(list(&s)),
                _ => c.channels = list(&s).iter().filter_map(|x| x.parse().ok()).collect(),
            },
            Err(e) => c.warnings.push(format!("{command}: {e}")),
        }
    }
    match query(resource, timeout, &format!("SENS{channel}:CLAS:NAME?")) {
        Ok(s) => c.class = s.trim_matches(['"', '\'']).into(),
        Err(e) => c.warnings.push(e.to_string()),
    }
    match query(resource, timeout, &format!("CALC{channel}:PAR:CAT:EXT?")) {
        Ok(s) => match measurements(&s) {
            Ok(v) => c.measurements = v,
            Err(e) => c.warnings.push(e.to_string()),
        },
        Err(e) => c.warnings.push(e.to_string()),
    }
    let range = query(resource, timeout, "SYST:CAP:FREQ:MIN?")
        .and_then(|a| query(resource, timeout, "SYST:CAP:FREQ:MAX?").map(|b| (a, b)));
    match range {
        Ok((a, b)) => match (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
            (Ok(a), Ok(b)) if a.is_finite() && b.is_finite() && a > 0. && b > a => {
                c.frequency_range = Some((a, b))
            }
            _ => c.warnings.push("Plage de fréquence indéterminée".into()),
        },
        Err(e) => c.warnings.push(e.to_string()),
    }
    Ok(c)
}
pub fn is_pna(idn: &str) -> bool {
    let parts: Vec<_> = idn.split(',').collect();
    parts.len() >= 2
        && (parts[0].to_ascii_uppercase().contains("KEYSIGHT")
            || parts[0].to_ascii_uppercase().contains("AGILENT"))
        && ["N52", "E83"]
            .iter()
            .any(|p| parts[1].trim().to_ascii_uppercase().starts_with(p))
}
/// Streamline USB VNAs share the Keysight VNA SCPI command set. This profile
/// is deliberately not advertised as a universal driver for every USB VNA.
pub fn is_usb_vna(idn: &str) -> bool {
    let parts: Vec<_> = idn.split(',').collect();
    parts.len() >= 2
        && (parts[0].to_ascii_uppercase().contains("KEYSIGHT")
            || parts[0].to_ascii_uppercase().contains("AGILENT"))
        && ["P50", "P93"]
            .iter()
            .any(|p| parts[1].trim().to_ascii_uppercase().starts_with(p))
}
pub fn decode(bytes: &[u8], precision: u8) -> Result<Vec<f64>> {
    let size = match precision {
        32 => 4,
        64 => 8,
        _ => return Err(Error::Protocol("REAL32/64 attendu".into())),
    };
    if bytes.is_empty() || !bytes.len().is_multiple_of(size) || bytes.len() / size > MAX_POINTS * 2
    {
        return Err(Error::Protocol("Taille de tableau binaire invalide".into()));
    }
    let values: Vec<f64> = bytes
        .chunks_exact(size)
        .map(|b| {
            if size == 4 {
                f32::from_le_bytes(b.try_into().unwrap()) as f64
            } else {
                f64::from_le_bytes(b.try_into().unwrap())
            }
        })
        .collect();
    if values.iter().any(|v| !v.is_finite()) {
        return Err(Error::Protocol("Donnée instrument non finie".into()));
    }
    Ok(values)
}

pub(crate) fn saved_format(s: &mut dyn Session) -> Result<(String, String)> {
    let format = s.query("FORM:DATA?")?.to_ascii_uppercase();
    let parts: Vec<_> = format
        .trim()
        .trim_matches('"')
        .split(',')
        .map(str::trim)
        .collect();
    let bits = parts.get(1).and_then(|v| v.parse::<f64>().ok());
    let format = match (parts.as_slice(), bits) {
        (["ASC" | "ASCII", _], Some(0.)) => "ASC,0",
        (["REAL", _], Some(32.)) => "REAL,32",
        (["REAL", _], Some(64.)) => "REAL,64",
        _ => {
            return Err(Error::Protocol(
                "Format de transfert initial non reconnu".into(),
            ));
        }
    };
    let border = s.query("FORM:BORD?")?.to_ascii_uppercase();
    let border = match border.trim() {
        "NORM" | "NORMAL" => "NORM",
        "SWAP" => "SWAP",
        _ => return Err(Error::Protocol("Byte order initial non reconnu".into())),
    };
    Ok((format.into(), border.into()))
}
pub(crate) fn finish_transfer<T>(
    s: &mut dyn Session,
    format: &str,
    border: &str,
    result: Result<T>,
) -> Result<T> {
    // Attempt both restores, and retain restoration errors even if acquisition failed.
    let mut errors = Vec::new();
    for command in [format!("FORM:DATA {format}"), format!("FORM:BORD {border}")] {
        if let Err(e) = s.write(&command) {
            errors.push(format!("{command}: {e}"));
        }
    }
    if errors.is_empty() {
        return result;
    }
    let restoration = format!(
        "Échec de restauration du transfert : {}",
        errors.join(" ; ")
    );
    Err(Error::Protocol(match result {
        Ok(_) => restoration,
        Err(e) => format!("{e} ; {restoration}"),
    }))
}
/// Read the existing selected standard-channel measurement, optionally configure
/// the sweep and trigger. The live catalog validates the trace's parameter.
pub fn acquire(session: &mut dyn Session, c: &Config) -> Result<NetworkTrace> {
    acquire_vna(session, c, Kind::Pna)
}
pub fn acquire_vna(session: &mut dyn Session, c: &Config, kind: Kind) -> Result<NetworkTrace> {
    c.instrument.validate().map_err(Error::Protocol)?;
    let max = kind
        .max_rf_ports()
        .ok_or_else(|| Error::Protocol("Profil VNA requis".into()))?;
    let (receiver, source) = rf_core::physical::s_parameter_ports(&c.s_parameter)
        .ok_or_else(|| Error::Protocol("Paramètre Sij invalide".into()))?;
    if c.instrument.port_count > max
        || receiver > c.instrument.port_count
        || source > c.instrument.port_count
    {
        return Err(Error::Protocol("Paramètre S hors ports configurés".into()));
    }
    let idn = session.query("*IDN?")?;
    let identified = if kind == Kind::UsbVna {
        is_usb_vna(&idn)
    } else {
        is_pna(&idn)
    };
    if !identified
        || (!c.instrument.expected_idn.trim().is_empty()
            && !idn
                .to_lowercase()
                .contains(&c.instrument.expected_idn.trim().to_lowercase()))
    {
        return Err(Error::Protocol(
            "Identité incompatible avec le pilote PNA".into(),
        ));
    }
    let actual_ports = session
        .query("SYST:CAP:HARD:PORT:COUN?")?
        .trim()
        .parse::<u8>()
        .map_err(|_| Error::Protocol("Nombre de ports matériel indéterminé".into()))?;
    if c.instrument.port_count > actual_ports || receiver > actual_ports || source > actual_ports {
        return Err(Error::Protocol(
            "Le VNA réel ne possède pas tous les ports configurés".into(),
        ));
    }
    let ch = c.instrument.channel;
    let class = session.query(&format!("SENS{ch}:CLAS:NAME?"))?;
    if class.trim_matches(['"', '\'']) != "Standard" {
        return Err(Error::Protocol("Le port Paramètres S exige un canal Standard ; utiliser la fenêtre instrument pour les applications".into()));
    }
    let catalog = measurements(&session.query(&format!("CALC{ch}:PAR:CAT:EXT?"))?)?;
    let matches: Vec<_> = catalog
        .iter()
        .filter(|(name, param)| {
            param == &c.s_parameter
                && (c.instrument.measurement.is_empty() || name == &c.instrument.measurement)
        })
        .collect();
    if matches.len() != 1 {
        return Err(Error::Protocol(format!(
            "Choisir une mesure unique {} du canal {ch}",
            c.s_parameter
        )));
    }
    let name = &matches[0].0;
    if name.contains(['"', '\'', ';', '\n', '\r', '\0']) {
        return Err(Error::Protocol("Nom de mesure incompatible".into()));
    }
    session.write(&format!("CALC{ch}:PAR:SEL \"{name}\",FAST"))?;
    if c.instrument.configure_sweep {
        let min = session
            .query("SYST:CAP:FREQ:MIN?")?
            .trim()
            .parse::<f64>()
            .map_err(|e| Error::Protocol(e.to_string()))?;
        let max = session
            .query("SYST:CAP:FREQ:MAX?")?
            .trim()
            .parse::<f64>()
            .map_err(|e| Error::Protocol(e.to_string()))?;
        if !min.is_finite()
            || !max.is_finite()
            || !c.start_hz.is_finite()
            || !c.stop_hz.is_finite()
            || c.start_hz < min
            || c.stop_hz > max
            || c.stop_hz <= c.start_hz
            || !(2..=MAX_POINTS).contains(&c.points)
        {
            return Err(Error::Protocol(
                "Balayage hors plage de fréquence / taille supportée".into(),
            ));
        }
        configure_power_and_cal(session, c)?;
        session.write(&format!("SENS{ch}:SWE:TYPE LIN"))?;
        for cmd in [
            format!("SENS{ch}:FREQ:STAR {}", c.start_hz),
            format!("SENS{ch}:FREQ:STOP {}", c.stop_hz),
            format!("SENS{ch}:SWE:POIN {}", c.points),
            format!("SENS{ch}:BWID {}", c.instrument.if_bandwidth_hz),
            format!("SENS{ch}:AVER:COUN {}", c.instrument.averages),
            format!("SENS{ch}:AVER {}", u8::from(c.instrument.averaging)),
        ] {
            session.write(&cmd)?;
        }
    }
    if c.instrument.trigger {
        session.write(&format!("SENS{ch}:SWE:MODE HOLD"))?;
        session.write(&format!("INIT{ch}:IMM"))?;
        if session.query("*OPC?")?.trim() != "1" {
            return Err(Error::Protocol("Balayage non terminé".into()));
        }
    }
    let (old_format, old_border) = saved_format(session)?;
    let result = (|| {
        session.write("FORM:BORD SWAP")?;
        session.write("FORM:DATA REAL,64")?;
        let frequency_hz = decode(&session.read_binary(&format!("CALC{ch}:X?"))?, 64)?;
        session.write(&format!("FORM:DATA REAL,{}", c.instrument.precision))?;
        let values = decode(
            &session.read_binary(&format!("CALC{ch}:DATA? SDATA"))?,
            c.instrument.precision,
        )?;
        if frequency_hz.len() < 2
            || frequency_hz.len() > MAX_POINTS
            || values.len() != 2 * frequency_hz.len()
            || frequency_hz.iter().any(|f| *f <= 0.)
            || frequency_hz.windows(2).any(|w| w[1] <= w[0])
        {
            return Err(Error::Protocol(
                "Axe fréquence et SDATA incompatibles (balayage fréquence requis)".into(),
            ));
        }
        let mut magnitude_db = Vec::new();
        let mut phase_deg = Vec::new();
        for z in values.chunks_exact(2) {
            let mag = z[0].hypot(z[1]);
            // Preserve a finite display floor for exact zeros; matrix operations
            // independently reject vanishing transmission before inversion.
            magnitude_db.push(20. * mag.max(1e-15).log10());
            phase_deg.push(z[1].atan2(z[0]).to_degrees());
        }
        Ok(NetworkTrace {
            frequency_hz,
            magnitude_db,
            phase_deg,
            parameter: c.s_parameter.clone(),
            simulated: false,
        })
    })();
    finish_transfer(session, &old_format, &old_border, result)
}
