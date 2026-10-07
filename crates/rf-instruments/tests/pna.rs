use rf_core::Config;
use rf_instruments::{ResourceManager, Result, Session, pna};
use std::{collections::BTreeMap, time::Duration};
#[derive(Default)]
struct Mock {
    commands: Vec<String>,
    responses: BTreeMap<String, String>,
    binary: BTreeMap<String, Vec<u8>>,
    failed_writes: Vec<String>,
}
impl Session for Mock {
    fn write(&mut self, c: &str) -> Result<()> {
        self.commands.push(c.into());
        if self.failed_writes.iter().any(|v| v == c) {
            Err(rf_instruments::Error::Protocol(
                "transport interrompu".into(),
            ))
        } else {
            Ok(())
        }
    }
    fn query(&mut self, c: &str) -> Result<String> {
        self.commands.push(c.into());
        Ok(self.responses.get(c).cloned().expect(c))
    }
    fn read_binary(&mut self, c: &str) -> Result<Vec<u8>> {
        self.commands.push(c.into());
        Ok(self.binary.get(c).cloned().expect(c))
    }
}
fn mock() -> Mock {
    Mock {
        responses: BTreeMap::from([
            ("SYST:CAP:HARD:PORT:COUN?".into(), "4".into()),
            (
                "*IDN?".into(),
                "Keysight Technologies,N5245B,MY123,A.15".into(),
            ),
            ("SENS3:CLAS:NAME?".into(), "\"Standard\"".into()),
            (
                "CALC3:PAR:CAT:EXT?".into(),
                "\"gain,S21,return,S11\"".into(),
            ),
            ("FORM:DATA?".into(), "ASC,0".into()),
            ("FORM:BORD?".into(), "NORM".into()),
        ]),
        binary: BTreeMap::from([
            (
                "CALC3:X?".into(),
                [36e9_f64, 37e9, 38e9]
                    .into_iter()
                    .flat_map(f64::to_le_bytes)
                    .collect(),
            ),
            (
                "CALC3:DATA? SDATA".into(),
                [10_f32, 0., 0., 10., -10., 0.]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect(),
            ),
        ]),
        ..Default::default()
    }
}
fn config() -> Config {
    let mut c = Config::default();
    c.instrument.channel = 3;
    c.instrument.measurement = "gain".into();
    c
}
#[test]
fn usb_identity_and_four_port_s_parameters_are_bounded_by_real_hardware() {
    let mut c = config();
    c.instrument.port_count = 4;
    c.s_parameter = "S43".into();
    let mut s = mock();
    s.responses
        .insert("CALC3:PAR:CAT:EXT?".into(), "gain,S43".into());
    assert_eq!(
        pna::acquire_vna(&mut s, &c, rf_core::Kind::PnaX)
            .unwrap()
            .parameter,
        "S43"
    );
    let mut s = mock();
    s.responses
        .insert("SYST:CAP:HARD:PORT:COUN?".into(), "2".into());
    assert!(
        pna::acquire_vna(&mut s, &c, rf_core::Kind::PnaX)
            .unwrap_err()
            .to_string()
            .contains("ports configurés")
    );
    assert!(!s.commands.iter().any(|s| s.starts_with("CALC3:PAR:SEL")));
    let c = config();
    let mut s = mock();
    s.responses.insert(
        "*IDN?".into(),
        "Keysight Technologies,P9374A,MY123,A.15".into(),
    );
    assert!(pna::acquire_vna(&mut s, &c, rf_core::Kind::UsbVna).is_ok());
    let mut s = mock();
    assert!(pna::acquire_vna(&mut s, &c, rf_core::Kind::UsbVna).is_err());
    assert_eq!(s.commands, ["*IDN?"]);
}
#[test]
fn catalog_and_license_checks() {
    assert_eq!(pna::measurements("\"a,S21,b,S11\"").unwrap().len(), 2);
    assert_eq!(pna::measurements("'a,b',S21").unwrap()[0].0, "a,b");
    assert!(pna::measurements("a,S21,odd").is_err());
    let mut cap = pna::Capabilities {
        options: "086,029,090".into(),
        ..Default::default()
    };
    assert!(!cap.supports("Gain Compression"));
    cap.valid_classes = Some(vec!["Standard".into(), "Gain Compression".into()]);
    assert!(cap.supports("Gain Compression"));
    assert!(!cap.supports("Spectrum Analyzer"));
}
#[test]
fn binary_channel_axes_precision_and_restore() {
    let mut s = mock();
    let t = pna::acquire(&mut s, &config()).unwrap();
    assert_eq!(t.frequency_hz, [36e9, 37e9, 38e9]);
    assert_eq!(t.magnitude_db, [20., 20., 20.]);
    assert_eq!(t.phase_deg, [0., 90., 180.]);
    assert!(!t.simulated);
    assert!(s.commands.contains(&"CALC3:PAR:SEL \"gain\",FAST".into()));
    assert!(s.commands.contains(&"FORM:DATA REAL,32".into()));
    assert_eq!(
        &s.commands[s.commands.len() - 2..],
        ["FORM:DATA ASC,0", "FORM:BORD NORM"]
    );
    assert!(
        !s.commands
            .iter()
            .any(|c| c.starts_with("INIT") || c.contains("OUTP") || c.contains("FREQ:STAR"))
    );
    let mut s = mock();
    s.binary.insert(
        "CALC3:DATA? SDATA".into(),
        [10_f64, 0., 0., 10., -10., 0.]
            .into_iter()
            .flat_map(f64::to_le_bytes)
            .collect(),
    );
    let mut c = config();
    c.instrument.precision = 64;
    assert_eq!(
        pna::acquire(&mut s, &c).unwrap().magnitude_db,
        [20., 20., 20.]
    );
}
#[test]
fn rejects_wrong_trace_class_payload_and_restores_on_bad_values() {
    let mut s = mock();
    let mut c = config();
    c.s_parameter = "S12".into();
    assert!(pna::acquire(&mut s, &c).is_err());
    assert!(!s.commands.iter().any(|c| c.starts_with("FORM:")));
    let mut s = mock();
    s.responses
        .insert("SENS3:CLAS:NAME?".into(), "Gain Compression".into());
    assert!(pna::acquire(&mut s, &config()).is_err());
    let mut s = mock();
    s.binary.insert("CALC3:DATA? SDATA".into(), vec![0; 4]);
    assert!(pna::acquire(&mut s, &config()).is_err());
    assert_eq!(s.commands.last().unwrap(), "FORM:BORD NORM");
    let mut s = mock();
    s.binary.insert("CALC3:DATA? SDATA".into(), vec![0; 4]);
    s.failed_writes.push("FORM:DATA ASC,0".into());
    let error = pna::acquire(&mut s, &config()).unwrap_err().to_string();
    assert!(error.contains("Axe fréquence"));
    assert!(error.contains("restauration"));
    assert_eq!(s.commands.last().unwrap(), "FORM:BORD NORM");
    assert!(pna::decode(&[0; 3], 32).is_err());
    assert!(pna::decode(&f64::NAN.to_le_bytes(), 64).is_err());
    let mut s = mock();
    s.responses
        .insert("FORM:DATA?".into(), "REAL,32;OUTP ON".into());
    assert!(pna::acquire(&mut s, &config()).is_err());
    assert!(!s.commands.iter().any(|c| c.contains("OUTP")));
}
#[test]
fn tcp_fragmented_binary_acquisition() {
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut r = BufReader::new(stream);
        let mut m = mock();
        loop {
            let mut cmd = String::new();
            if r.read_line(&mut cmd).unwrap() == 0 {
                break;
            }
            let cmd = cmd.trim();
            if let Some(v) = m.responses.remove(cmd) {
                writeln!(r.get_mut(), "{v}").unwrap();
            } else if let Some(v) = m.binary.remove(cmd) {
                let length = v.len().to_string();
                let mut block = format!("#{}{length}", length.len()).into_bytes();
                block.extend(v);
                block.push(b'\n');
                for part in block.chunks(3) {
                    r.get_mut().write_all(part).unwrap();
                }
            }
        }
    });
    let mut session = ResourceManager
        .open_resource(
            &format!("TCPIP::127.0.0.1::{}::SOCKET", addr.port()),
            Duration::from_secs(2),
        )
        .unwrap();
    let t = pna::acquire(session.as_mut(), &config()).unwrap();
    assert_eq!(t.magnitude_db, [20., 20., 20.]);
    drop(session);
    server.join().unwrap();
}

#[test]
fn licensed_application_binary_data_and_gca_level() {
    use rf_instruments::pna_application as app;
    let mut s = mock();
    s.responses
        .insert("SENS3:CLAS:NAME?".into(), "Gain Compression".into());
    s.responses.insert(
        "SYST:MCL:VAL:CAT?".into(),
        "\"Standard,Gain Compression\"".into(),
    );
    s.responses.insert("CALC3:FORM?".into(), "MLOG".into());
    s.binary.insert(
        "CALC3:DATA? FDATA".into(),
        [20_f32, 19., 18.]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect(),
    );
    let data = app::acquire(&mut s, &config()).unwrap();
    assert_eq!(data.class, "Gain Compression");
    assert_eq!(data.y, [20., 19., 18.]);
    assert_eq!(data.display_format, "MLOG");
    assert_eq!(s.commands.last().unwrap(), "FORM:BORD NORM");
    s.responses.insert("SENS3:GCS:COMP:LEV?".into(), "1".into());
    app::set_compression(&mut s, &config()).unwrap();
    assert!(s.commands.contains(&"SENS3:GCS:COMP:LEV 1".into()));
    assert_eq!(s.commands.last().unwrap(), "SENS3:GCS:COMP:LEV?");
}
#[test]
fn unlicensed_or_wrong_application_blocks_configuration() {
    use rf_instruments::pna_application as app;
    let mut s = mock();
    s.responses
        .insert("SENS3:CLAS:NAME?".into(), "Gain Compression".into());
    s.responses
        .insert("SYST:MCL:VAL:CAT?".into(), "Standard".into());
    assert!(app::set_compression(&mut s, &config()).is_err());
    assert!(!s.commands.iter().any(|c| c.contains("GCS:COMP:LEV")));
    let mut s = mock();
    s.responses
        .insert("SYST:MCL:VAL:CAT?".into(), "Standard".into());
    assert!(app::set_compression(&mut s, &config()).is_err());
    assert!(!s.commands.iter().any(|c| c.contains("GCS:COMP:LEV")));
}

#[test]
fn applied_sweep_checks_hardware_range_and_identity() {
    let mut c = config();
    c.start_hz = 36e9;
    c.stop_hz = 38e9;
    c.points = 3;
    c.instrument.configure_sweep = true;
    c.instrument.trigger = true;
    let mut s = mock();
    s.responses
        .insert("SYST:CAP:FREQ:MIN?".into(), "1e7".into());
    s.responses
        .insert("SYST:CAP:FREQ:MAX?".into(), "5e10".into());
    s.responses.insert("*OPC?".into(), "1".into());
    for port in 1..=2 {
        s.responses
            .insert(format!("SOUR3:POW{port}? MIN"), "-60".into());
        s.responses
            .insert(format!("SOUR3:POW{port}? MAX"), "10".into());
    }
    pna::acquire(&mut s, &c).unwrap();
    assert!(s.commands.contains(&"SENS3:FREQ:STAR 36000000000".into()));
    assert!(s.commands.contains(&"INIT3:IMM".into()));
    c.stop_hz = 51e9;
    let mut s = mock();
    s.responses
        .insert("SYST:CAP:FREQ:MIN?".into(), "1e7".into());
    s.responses
        .insert("SYST:CAP:FREQ:MAX?".into(), "5e10".into());
    assert!(pna::acquire(&mut s, &c).is_err());
    assert!(!s.commands.iter().any(|c| c.starts_with("SENS3:FREQ:STAR")));
    c.instrument.expected_idn = "wrong serial".into();
    let mut s = mock();
    assert!(pna::acquire(&mut s, &c).is_err());
    assert_eq!(s.commands, ["*IDN?"]);
}
#[test]
fn multi_trace_read_uses_one_trigger_and_validates_catalog_before_writes() {
    let mut c = config();
    c.instrument.pna.all_s_parameters = true;
    c.instrument.trigger = true;
    let mut s = mock();
    s.responses.insert(
        "CALC3:PAR:CAT:EXT?".into(),
        "\"gain,S21,return,S11,reverse,S12,output,S22\"".into(),
    );
    s.responses.insert("*OPC?".into(), "1".into());
    let curves = pna::acquire_curves(&mut s, &c, rf_core::Kind::PnaX, 7).unwrap();
    assert_eq!(curves.len(), 4);
    assert!(curves.iter().all(|c| c.node == 7 && c.x_unit == "Hz"));
    assert_eq!(s.commands.iter().filter(|s| *s == "INIT3:IMM").count(), 1);
    let mut s = mock();
    assert!(pna::acquire_curves(&mut s, &c, rf_core::Kind::PnaX, 7).is_err());
    assert!(
        !s.commands
            .iter()
            .any(|c| c.starts_with("INIT") || c.starts_with("SOUR"))
    );
}
#[test]
fn power_axis_and_out_of_range_level_are_not_silently_accepted() {
    let mut c = config();
    c.instrument.pna.power_sweep = true;
    c.instrument.pna.power_start_dbm = -30.;
    c.instrument.pna.power_stop_dbm = -10.;
    c.instrument.configure_sweep = true;
    c.frequency_hz = 37e9;
    c.points = 3;
    let mut s = mock();
    for (key, value) in [
        ("SYST:CAP:FREQ:MIN?", "1e7"),
        ("SYST:CAP:FREQ:MAX?", "5e10"),
        ("SOUR3:POW1? MIN", "-60"),
        ("SOUR3:POW1? MAX", "10"),
    ] {
        s.responses.insert(key.into(), value.into());
    }
    s.binary.insert(
        "CALC3:X?".into(),
        [-30_f64, -20., -10.]
            .into_iter()
            .flat_map(f64::to_le_bytes)
            .collect(),
    );
    let r = pna::acquire_curves(&mut s, &c, rf_core::Kind::PnaX, 1).unwrap();
    assert_eq!(r[0].x_unit, "dBm");
    assert_eq!(r[0].x, [-30., -20., -10.]);
    assert!(s.commands.contains(&"SENS3:SWE:TYPE POW".into()));
    assert!(s.commands.contains(&"SOUR3:POW1:PORT:STOP -10".into()));
    assert!(s.commands.contains(&"FORM:DATA ASC,0".into()));
    s.commands.clear();
    c.instrument.pna.power_stop_dbm = 20.;
    assert!(pna::acquire_curves(&mut s, &c, rf_core::Kind::PnaX, 1).is_err());
    assert!(!s.commands.iter().any(|c| c.starts_with("SOUR3:POW1:PORT")));
}
