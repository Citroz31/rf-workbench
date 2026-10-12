use rf_core::{Graph, Kind};
use rf_runtime::Engine;
use std::sync::atomic::AtomicBool;
fn run(g: &Graph) -> rf_runtime::RunResult {
    Engine::default().execute(g, 0, "unused", false, &AtomicBool::new(false)).unwrap()
}
fn bench(reverse: bool) -> (Graph, u64, u64) {
    let mut g = Graph::default();
    let v = g.add(Kind::PnaX, [0., 0.]);
    let d = g.add(Kind::Dut, [320., 0.]);
    g.nodes[0].config.instrument.pna.all_s_parameters = true;
    g.nodes[1].config.loss_db = -26.;
    g.nodes[1].config.dut.s11_db = -12.;
    g.nodes[1].config.dut.s22_db = -18.;
    g.nodes[1].config.dut.s12_db = -47.;
    g.nodes[1].config.phase_deg = 45.;
    g.connect_physical(v, usize::from(reverse), d, 0).unwrap();
    g.connect_physical(v, usize::from(!reverse), d, 1).unwrap();
    (g, v, d)
}
#[test]
fn separate_dut_controls_all_s_parameters_and_edits_apply_next_run() {
    let (mut g, _, _) = bench(false);
    let r = run(&g);
    for (p, expected) in [("S11", -12.), ("S22", -18.), ("S21", 26.), ("S12", -47.)] {
        assert!(r.curves.iter().find(|c| c.name == p).unwrap().y.iter().all(|v| (*v-expected).abs()<1e-10));
    }
    g.nodes[1].config.loss_db = -19.;
    g.nodes[1].config.attenuation_db = 2.;
    assert_eq!(run(&g).curves.iter().find(|c| c.name == "S21").unwrap().y[0], 17.);
}
#[test]
fn reversed_cabling_maps_reflections_and_forward_gain_to_electrical_ports() {
    let (mut g, _, _) = bench(true);
    g.nodes[0].rotation = 1;
    g.nodes[1].rotation = 3;
    let r = run(&g);
    for (p, expected) in [("S11", -18.), ("S22", -12.), ("S12", 26.), ("S21", -47.)] {
        assert_eq!(r.curves.iter().find(|c| c.name == p).unwrap().y[0], expected);
    }
}
#[test]
fn empty_vna_does_not_invent_an_internal_dut() {
    let mut g = Graph::default();
    g.add(Kind::PnaX, [0., 0.]);
    let err = Engine::default().execute(&g,0,"unused",false,&AtomicBool::new(false)).unwrap_err();
    assert!(err.contains("simulation sans DUT"));
}
#[test]
fn edited_dut_band_blocks_incompatible_vna_sweep() {
    let (mut g, _, _) = bench(false);
    g.nodes[1].config.limits.band_hz = Some([8e9, 12e9]);
    assert!(rf_runtime::preflight(&g).blocked());
}
