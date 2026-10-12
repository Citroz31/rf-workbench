//! Stateful SCPI peer: checks the composed configuration/acquisition contract.
use rf_core::{
    Config, Kind,
    pna_plan::{Format, Plan},
};
use rf_instruments::{Result, Session, pna, pna_path};
use std::collections::BTreeMap;
struct Peer {
    commands: Vec<String>,
    measurements: BTreeMap<String, String>,
    selected: String,
    formats: BTreeMap<String, String>,
    units: BTreeMap<String, String>,
    windows: BTreeMap<u32, BTreeMap<u32, String>>,
    settings: BTreeMap<String, String>,
    elements: BTreeMap<String, String>,
    transfer: u8,
    clamp_if: bool,
}
impl Default for Peer {
    fn default() -> Self {
        Self {
            commands: vec![],
            measurements: BTreeMap::new(),
            selected: String::new(),
            formats: BTreeMap::new(),
            units: BTreeMap::new(),
            windows: BTreeMap::new(),
            settings: BTreeMap::from([
                ("SENS3:SWE:TYPE".into(), "LIN".into()),
                ("SENS3:SWE:POIN".into(), "3".into()),
                ("SENS3:FREQ:STAR".into(), "2400000000".into()),
                ("SENS3:FREQ:STOP".into(), "2500000000".into()),
                ("SENS3:BWID".into(), "1000".into()),
                ("SOUR3:POW".into(), "-10".into()),
            ]),
            elements: BTreeMap::from([
                ("Combiner".into(), "Normal".into()),
                ("Port1Bypass".into(), "Thru".into()),
            ]),
            transfer: 32,
            clamp_if: false,
        }
    }
}
impl Session for Peer {
    fn write(&mut self, c: &str) -> Result<()> {
        self.commands.push(c.into());
        if let Some(args) = c.strip_prefix("CALC3:PAR:DEF:EXT ") {
            let pair = pna::measurements(args).unwrap().remove(0);
            self.measurements
                .insert(pair.0, pair.1.replace(',', "_").replace(' ', ""));
        } else if let Some(name) = c.strip_prefix("CALC3:PAR:SEL ") {
            self.selected = name.split('"').nth(1).unwrap().into();
        } else if let Some(format) = c.strip_prefix("CALC3:FORM ") {
            self.formats.insert(self.selected.clone(), format.into());
        } else if c.starts_with("CALC3:FORM:UNIT ") {
            self.units.insert(self.selected.clone(), "DBM".into());
        } else if c.starts_with("SENS3:PATH:CONF:SEL ") {
            self.settings.insert("SENS3:BWID".into(), "777".into());
        } else if let Some(args) = c.strip_prefix("SENS3:PATH:CONF:ELEM:SEL ") {
            let pair = pna::measurements(args).unwrap().remove(0);
            self.elements.insert(pair.0, pair.1);
        } else if let Some(p) = c.strip_prefix("FORM:DATA REAL,") {
            self.transfer = p.parse().unwrap();
        } else if c.starts_with("DISP:WIND") && c.contains(":FEED ") {
            let win = c
                .strip_prefix("DISP:WIND")
                .unwrap()
                .split(':')
                .next()
                .unwrap()
                .parse()
                .unwrap();
            let trace = c
                .split(":TRAC")
                .nth(1)
                .unwrap()
                .split(':')
                .next()
                .unwrap()
                .parse()
                .unwrap();
            self.windows
                .entry(win)
                .or_default()
                .insert(trace, c.split('"').nth(1).unwrap().into());
        } else if let Some((key, value)) = c.split_once(' ')
            && (key.starts_with("SENS3:") || key.starts_with("SOUR3:"))
        {
            self.settings.insert(
                key.into(),
                if self.clamp_if && key == "SENS3:BWID" {
                    "999".into()
                } else {
                    value.into()
                },
            );
        }
        Ok(())
    }
    fn query(&mut self, c: &str) -> Result<String> {
        self.commands.push(c.into());
        Ok(match c {
            "*IDN?" => "Keysight Technologies,N5245B,MYTEST,A.15".into(),
            "SYST:CAP:HARD:PORT:COUN?" => "4".into(),
            "SENS3:CLAS:NAME?" => "Standard".into(),
            "SYST:CAP:FREQ:MIN?" => "10000000".into(),
            "SYST:CAP:FREQ:MAX?" => "50000000000".into(),
            "SYST:ERR?" => "0,No error".into(),
            "*OPC?" => "1".into(),
            "FORM:DATA?" => "ASC,0".into(),
            "FORM:BORD?" => "NORM".into(),
            "SENS3:SWE:MODE?" => "HOLD".into(),
            "CALC3:PAR:CAT:EXT?" => self
                .measurements
                .iter()
                .map(|(n, p)| format!("\"{n}\",\"{p}\""))
                .collect::<Vec<_>>()
                .join(","),
            "CALC3:FORM?" => self
                .formats
                .get(&self.selected)
                .cloned()
                .unwrap_or("MLOG".into()),
            "CALC3:FORM:UNIT? MLOG" => "DBM".into(),
            "SENS3:PATH:CONF:ELEM:CAT?" => "Combiner,Port1Bypass".into(),
            "SENS:PATH:CONF:CAT?" => "Default,HighPowerTest".into(),
            "SENS3:PATH:CONF:NAME?" => "Default".into(),
            "SENS3:PATH:CONF:DTEX?" => "Connect jumpers manually".into(),
            _ if c.starts_with("SENS3:PATH:CONF:ELEM:VAL:CAT?") => if c.contains("Combiner") {
                "Normal,Reversed"
            } else {
                "Thru,Combiner"
            }
            .into(),
            _ if c.starts_with("SENS3:PATH:CONF:ELEM?") => {
                self.elements[c.split('"').nth(1).unwrap()].clone()
            }
            _ if c.starts_with("DISP:WIND") && c.ends_with(":CAT?") => {
                let win: u32 = c
                    .strip_prefix("DISP:WIND")
                    .unwrap()
                    .split(':')
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap();
                self.windows
                    .get(&win)
                    .map(|w| w.keys().map(u32::to_string).collect::<Vec<_>>().join(","))
                    .unwrap_or("EMPTY".into())
            }
            _ if c.ends_with("? MIN") => "-90".into(),
            _ if c.ends_with("? MAX") => "20".into(),
            _ => self
                .settings
                .get(c.trim_end_matches('?'))
                .unwrap_or_else(|| panic!("unexpected query {c}"))
                .clone(),
        })
    }
    fn read_binary(&mut self, c: &str) -> Result<Vec<u8>> {
        self.commands.push(c.into());
        let parameter = self.measurements.get(&self.selected).unwrap();
        let pow = self.settings["SENS3:SWE:TYPE"] == "POW";
        let values = if c.ends_with(":X?") {
            if pow {
                vec![-30., -20., -10.]
            } else {
                vec![2.4e9, 2.45e9, 2.5e9]
            }
        } else if c.ends_with("FDATA") {
            vec![-30., -20., -10.]
                .into_iter()
                .map(|p| p + if parameter.starts_with('R') { 0. } else { 20. })
                .collect()
        } else {
            let mag = if parameter == "S21" { 10. } else { 0.1 };
            vec![mag, 0., mag, 0., mag, 0.]
        };
        Ok(if c.ends_with(":X?") || self.transfer == 64 {
            values.into_iter().flat_map(f64::to_le_bytes).collect()
        } else {
            values
                .into_iter()
                .flat_map(|v| (v as f32).to_le_bytes())
                .collect()
        })
    }
}
fn config() -> Config {
    let mut c = Config::default();
    c.instrument.channel = 3;
    c.points = 3;
    c.instrument.pna.plan = Some(Plan::default());
    c.instrument.configure_sweep = true;
    c.instrument.trigger = true;
    c
}
#[test]
fn s_layout_and_acquisition_share_one_channel_and_one_sweep() {
    let mut s = Peer::default();
    let mut c = config();
    c.instrument.pna.path.apply = true;
    c.instrument.pna.path.configuration = "HighPowerTest".into();
    c.instrument
        .pna
        .path
        .elements
        .insert("Combiner".into(), "Reversed".into());
    let data = pna::acquire_curves(&mut s, &c, Kind::PnaX, 1).unwrap();
    assert_eq!(
        data.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
        ["S11", "S12", "S21", "S22"]
    );
    assert_eq!(s.commands.iter().filter(|c| *c == "INIT3:IMM").count(), 1);
    assert!(
        s.commands
            .iter()
            .all(|c| !c.contains("PRESet") && !c.starts_with("DISP:SPL"))
    );
    for (i, p) in ["S11", "S12", "S21", "S22"].iter().enumerate() {
        let name = &s.windows[&(i as u32 + 1)][&1];
        assert_eq!(&s.measurements[name], p);
    }
    let recall = s
        .commands
        .iter()
        .position(|c| c.starts_with("SENS3:PATH:CONF:SEL "))
        .unwrap();
    let ifbw = s
        .commands
        .iter()
        .position(|c| c == "SENS3:BWID 1000")
        .unwrap();
    assert!(recall < ifbw);
}
#[test]
fn power_plan_reads_gain_and_actual_receiver_fdata_from_one_sweep() {
    let mut c = config();
    c.instrument.pna.power_sweep = true;
    c.instrument.pna.plan = Some(Plan::power(1, 2));
    let mut s = Peer::default();
    let curves = pna::acquire_curves(&mut s, &c, Kind::PnaX, 1).unwrap();
    assert_eq!(curves.len(), 4);
    assert!(curves.iter().all(|t| t.x_unit == "dBm"));
    assert_eq!(
        curves.iter().find(|t| t.name == "B_1").unwrap().y,
        [-10., 0., 10.]
    );
    assert_eq!(
        curves.iter().find(|t| t.name == "R1_1").unwrap().y,
        [-30., -20., -10.]
    );
    assert_eq!(s.commands.iter().filter(|c| *c == "INIT3:IMM").count(), 1);
    assert_eq!(
        s.commands
            .iter()
            .filter(|c| *c == "CALC3:DATA? FDATA")
            .count(),
        2
    );
}
#[test]
fn unknown_path_and_clamped_settings_are_rejected_before_trigger() {
    let mut c = config();
    c.instrument.pna.path.apply = true;
    c.instrument
        .pna
        .path
        .elements
        .insert("Unavailable".into(), "Thru".into());
    let mut s = Peer::default();
    assert!(pna::acquire_curves(&mut s, &c, Kind::PnaX, 1).is_err());
    assert!(!s.commands.iter().any(|c| c.starts_with("INIT")));
    c.instrument.pna.path.apply = false;
    let mut s = Peer {
        clamp_if: true,
        ..Peer::default()
    };
    assert!(
        pna::acquire_curves(&mut s, &c, Kind::PnaX, 1)
            .unwrap_err()
            .to_string()
            .contains("accepté")
    );
    assert!(!s.commands.iter().any(|c| c.starts_with("INIT")));
}
#[test]
fn overlay_formats_and_manual_scales_are_independent() {
    let mut c = config();
    let plan = c.instrument.pna.plan.as_mut().unwrap();
    plan.parameters = vec!["S11".into(), "S22".into()];
    plan.rebuild_windows();
    let other = plan.windows.pop().unwrap();
    plan.windows[0].traces.extend(other.traces);
    plan.windows[0].traces[0].format = Format::Phase;
    plan.windows[0].traces[0].automatic = false;
    plan.windows[0].traces[0].per_division = 2.;
    plan.windows[0].traces[1].automatic = false;
    plan.windows[0].traces[1].per_division = 10.;
    let mut s = Peer::default();
    pna::configure(&mut s, &c, Kind::PnaX).unwrap();
    assert!(s.commands.contains(&"DISP:WIND1:TRAC1:Y:PDIV 2".into()));
    assert!(s.commands.contains(&"DISP:WIND1:TRAC2:Y:PDIV 10".into()));
    assert!(s.commands.contains(&"CALC3:FORM PHAS".into()));
    assert!(s.commands.contains(&"DISP:WIND1:TRAC:Y:COUP OFF".into()));
    assert!(!s.commands.iter().any(|c| c.starts_with("INIT")));
}
#[test]
fn live_path_catalogue_and_stale_sweep_are_checked() {
    let mut s = Peer::default();
    let mut c = config();
    let snapshot = pna_path::inspect(&mut s, &c).unwrap();
    assert_eq!(snapshot.elements.len(), 2);
    c.instrument.configure_sweep = false;
    for p in ["S11", "S12", "S21", "S22"] {
        s.measurements.insert(p.into(), p.into());
    }
    s.settings
        .insert("SENS3:FREQ:STAR".into(), "36000000000".into());
    assert!(pna::acquire_curves(&mut s, &c, Kind::PnaX, 1).is_err());
    assert!(!s.commands.iter().any(|c| c.starts_with("INIT")));
}
