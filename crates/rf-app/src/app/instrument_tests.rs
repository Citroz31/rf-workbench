use super::instrument::Dialog;
use rf_core::{Graph, Kind};
use rf_instruments::discovery::Device;
#[test]
fn automatic_address_requires_unique_identity_and_keeps_manual_selection() {
    let mut g = Graph::default();
    let data_directory = tempfile::tempdir().unwrap();
    let id = g.add(Kind::PnaX, [0., 0.]);
    let mut d = Dialog::new(g.node(id).unwrap(), data_directory.path());
    assert_eq!(
        std::path::Path::new(&d.binary_path),
        data_directory.path().join("instrument-data.bin")
    );
    let device = |resource: &str| Device {
        resource: resource.into(),
        idn: "Keysight,N5245B,MY123,A.15".into(),
        error: None,
    };
    d.accept_devices(vec![
        device("GPIB0::16::INSTR"),
        device("USB0::0::1::INSTR"),
    ]);
    assert_eq!(d.config.resource, "SIM::RF::INSTR");
    d.accept_devices(vec![device("GPIB0::16::INSTR")]);
    assert_eq!(d.config.resource, "GPIB0::16::INSTR");
    d.accept_devices(vec![device("USB0::0::1::INSTR")]);
    assert_eq!(d.config.resource, "GPIB0::16::INSTR");
    let id = g.add(Kind::Thermometer, [0., 0.]);
    let mut d = Dialog::new(g.node(id).unwrap(), data_directory.path());
    d.config.instrument.expected_idn = "MY123".into();
    d.accept_devices(vec![device("GPIB0::16::INSTR")]);
    assert_eq!(d.config.resource, "GPIB0::16::INSTR");
}
