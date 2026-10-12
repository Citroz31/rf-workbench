use rf_core::{Graph,Kind,pna_plan::Plan};
use rf_runtime::Engine;
use std::sync::atomic::AtomicBool;
fn bench(reverse:bool)->Graph {
    let catalog=rf_dut_library::Catalog::default();let component=&catalog.entries[0];
    let mut g=Graph::default();let v=g.add(Kind::PnaX,[0.,0.]);let d=g.add(Kind::Dut,[300.,0.]);
    g.nodes[0].config.start_hz=27e9;g.nodes[0].config.stop_hz=31.5e9;g.nodes[0].config.points=19;
    g.nodes[1].config.dut.dataset=component.dataset.clone();g.nodes[1].config.dut.use_dataset=true;
    g.nodes[1].config.limits.band_hz=Some([27e9,31.5e9]);
    g.connect_physical(v,usize::from(reverse),d,0).unwrap();g.connect_physical(v,usize::from(!reverse),d,1).unwrap();g
}
fn run(g:&Graph)->rf_runtime::RunResult {Engine::default().execute(g,0,"unused",false,&AtomicBool::new(false)).unwrap()}
#[test] fn vna_replays_published_modules_without_fabricating_phase() {
    let g=bench(false);let result=run(&g);
    let dataset=g.nodes[1].config.dut.dataset.as_ref().unwrap();
    for p in ["S11","S12","S21","S22","NF"] {
        let curve=result.curves.iter().find(|c|c.name==p).unwrap();let published=dataset.curve(p).unwrap();
        assert_eq!(curve.y,published.values);assert!(curve.phase_deg.is_none());assert!(curve.simulated);
        assert_eq!(curve.provenance.as_ref().unwrap().page,4);
    }
    assert!(rf_runtime::two_port(&result.curves,g.nodes[0].id,50.).is_err());
    let json=rf_core::Project {schema_version:rf_core::SCHEMA_VERSION,name:"catalog".into(),graph:g}.to_json().unwrap();
    let loaded=rf_core::Project::from_json(&json).unwrap();assert!(loaded.graph.nodes[1].config.dut.use_dataset);
}
#[test] fn reversing_cables_maps_dataset_terms_and_nominal_attenuation() {
    let mut g=bench(true);g.nodes[1].config.attenuation_db=3.;let result=run(&g);
    let dataset=g.nodes[1].config.dut.dataset.as_ref().unwrap();
    let forward=result.curves.iter().find(|c|c.name=="S12").unwrap();
    assert!((forward.y[0]-(dataset.curve("S21").unwrap().values[0]-3.)).abs()<1e-10);
    assert_eq!(result.curves.iter().find(|c|c.name=="S11").unwrap().y,dataset.curve("S22").unwrap().values);
}
#[test] fn power_receivers_use_connected_dut_and_the_selected_source() {
    let mut g=bench(false);let c=&mut g.nodes[0].config;c.instrument.pna.power_sweep=true;c.instrument.pna.plan=Some(Plan::power(1,2));
    c.frequency_hz=29e9;c.points=3;c.instrument.pna.power_start_dbm=-30.;c.instrument.pna.power_stop_dbm=-10.;
    let result=run(&g);let gain=result.curves.iter().find(|c|c.name=="S21").unwrap();
    let input=result.curves.iter().find(|c|c.name=="R1_1").unwrap();let output=result.curves.iter().find(|c|c.name=="B_1").unwrap();
    assert_eq!(input.y,[-30.,-20.,-10.]);for i in 0..3 {assert!((output.y[i]-input.y[i]-gain.y[i]).abs()<1e-10);}
    g.nodes[0].config.instrument.pna.source_port=2;assert!(g.validate().is_err());
}
#[test] fn overload_return_is_checked_and_idle_source_ports_do_not_propagate() {
    let mut g=bench(false);g.nodes[1].config.dut.use_dataset=false;g.nodes[1].config.loss_db=-40.;
    g.nodes[0].config.power_dbm=0.;g.nodes[0].config.limits.max_input_dbm=Some(10.);
    assert!(rf_runtime::preflight(&g).blocked());
    g.nodes[0].config.instrument.pna.power_sweep=true;g.nodes[0].config.instrument.pna.source_port=2;
    g.nodes[0].config.frequency_hz=29e9;
    g.nodes[0].config.s_parameter="S12".into();g.nodes[0].config.instrument.pna.plan=Some(Plan::power(2,1));
    g.nodes[1].config.limits.max_output_dbm=Some(10.);
    assert!(!rf_runtime::preflight(&g).blocked());
}
