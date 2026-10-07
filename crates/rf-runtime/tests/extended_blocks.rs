use rf_core::{Graph, Kind};
use rf_runtime::Engine;
use std::sync::atomic::AtomicBool;
fn run(g: &Graph) -> rf_runtime::RunResult {
    Engine::default()
        .execute(g, 0, "unused", false, &AtomicBool::new(false))
        .unwrap()
}
#[test]
fn iq_chain_uses_both_awg_channels_and_shifts_lo() {
    let r = run(&Graph::iq_demo());
    let (frequency, level) = r.trace.unwrap().peak().unwrap();
    assert!((frequency - 2.451e9).abs() < 200_000.);
    assert!((level + 13.).abs() < 0.01);
    assert_eq!(r.completed.len(), 6);
    assert_eq!(r.waveform.unwrap().unit, "V");
}
#[test]
fn pna_and_pna_x_keep_s_parameters_out_of_dbm_traces() {
    for kind in [Kind::Pna, Kind::PnaX] {
        let mut g = Graph::network_demo();
        g.nodes[1].kind = kind;
        g.nodes[0].config.loss_db = 8.;
        let r = run(&g);
        assert!(r.trace.is_none());
        let t = r.network.unwrap();
        assert_eq!(t.parameter, "S21");
        assert!(t.simulated);
        assert_eq!(t.magnitude_db.len(), 401);
        assert!(t.magnitude_db.iter().all(|a| (-8.16..=-7.84).contains(a)));
        assert_eq!(t.phase_deg.len(), 401);
    }
}
#[test]
fn sensors_use_units_and_upstream_values() {
    let r = run(&rf_runtime::measurement_demo());
    for (unit, value) in [("°C", 85.), ("Ω", 50.), ("dBm", -10.), ("NF dB", 2.5)] {
        assert!(
            r.measurements
                .iter()
                .any(|m| m.unit == unit && m.value == value && m.simulated)
        );
    }
}
#[test]
fn adc_clips_and_quantizes_without_claiming_resampling() {
    let mut g = Graph::default();
    let a = g.add(Kind::Awg, [0., 0.]);
    let d = g.add(Kind::Dac, [300., 0.]);
    let c = g.add(Kind::Adc, [600., 0.]);
    g.connect(a, d).unwrap();
    g.connect(d, c).unwrap();
    g.nodes[1].config.voltage_v = 2.;
    g.nodes[2].config.resolution_bits = 4;
    let w = run(&g).waveform.unwrap();
    assert_eq!(w.unit, "FS");
    assert!(w.samples.iter().all(|v| (-1. ..=1.).contains(v)));
    let unique: std::collections::BTreeSet<_> = w
        .samples
        .iter()
        .map(|v| (((v + 1.) / 2.) * 15.).round() as i32)
        .collect();
    assert!(unique.len() <= 16);
    assert_eq!(w.sample_rate_hz, 100e6);
}
#[test]
fn mismatched_iq_cadences_and_hardware_profiles_fail_explicitly() {
    let mut g = Graph::iq_demo();
    g.edges.retain(|e| e.to != 3);
    let second_awg = g.add(Kind::Awg, [0., 600.]);
    g.nodes.last_mut().unwrap().config.tone_hz = 2e6;
    g.connect_ports(second_awg, 1, 3, 0).unwrap();
    assert!(
        Engine::default()
            .execute(&g, 0, "unused", false, &AtomicBool::new(false))
            .unwrap_err()
            .contains("fréquence")
    );
    let mut g = Graph::default();
    g.add(Kind::Awg, [0., 0.]);
    g.nodes[0].config.resource = "TCPIP::127.0.0.1::5025::SOCKET".into();
    assert!(
        Engine::default()
            .execute(&g, 0, "unused", true, &AtomicBool::new(false))
            .unwrap_err()
            .contains("profil matériel non implémenté")
    );
}

#[test]
fn pa_example_is_explicit_small_signal_simulation() {
    let g = Graph::pa_demo();
    let r = run(&g);
    let t = r.network.unwrap();
    assert!(t.simulated);
    assert_eq!(t.frequency_hz.first(), Some(&36e9));
    assert_eq!(t.frequency_hz.last(), Some(&38e9));
    assert!(t.magnitude_db.iter().all(|x| (19.8..=20.2).contains(x)));
    assert!(g.nodes[0].comment.contains("petit signal"));
}
