use rf_core::{Graph, Kind};
use rf_runtime::{Command, Engine, Event, Worker};
use std::sync::atomic::AtomicBool;
use std::time::Duration;
fn await_idle(w: &Worker) -> Vec<Event> {
    let mut events = Vec::new();
    loop {
        let e = w
            .events
            .recv_timeout(Duration::from_secs(4))
            .expect("worker completion");
        let idle = matches!(e, Event::Idle);
        events.push(e);
        if idle {
            break;
        }
    }
    events
}
#[test]
fn project_reset_clears_simulated_dc_output_and_sequence() {
    let w = Worker::spawn(|| {});
    let mut c = rf_core::Config::default();
    c.instrument.dc.voltage_v = 3.;
    for action in [
        rf_instruments::dc::Action::Apply,
        rf_instruments::dc::Action::Enable,
    ] {
        w.submit(Command::DcSupply {
            node: 1,
            kind: Kind::DcSupplyE36313A,
            config: c.clone(),
            action,
            hardware: false,
        })
        .unwrap();
        await_idle(&w);
    }
    w.submit(Command::ResetProject).unwrap();
    let events = await_idle(&w);
    assert!(
        events
            .iter()
            .any(|e| matches!(e,Event::ProjectReset(errors)if errors.is_empty()))
    );
    w.submit(Command::DcSupply {
        node: 1,
        kind: Kind::DcSupplyE36313A,
        config: c,
        action: rf_instruments::dc::Action::Read,
        hardware: false,
    })
    .unwrap();
    assert!(await_idle(&w).iter().any(|e|matches!(e,Event::InstrumentResponse{text,..}if text.contains("OFF")&&text.contains("0.000000 V"))));
    let mut g = Graph::network_demo();
    g.nodes
        .iter_mut()
        .find(|n| n.kind == Kind::PnaX)
        .unwrap()
        .config
        .instrument
        .pna
        .all_s_parameters = true;
    w.submit(Command::Run {
        graph: g,
        continuous: false,
        python_path: "unused".into(),
        hardware: false,
    })
    .unwrap();
    let events = await_idle(&w);
    let r = events
        .iter()
        .find_map(|e| {
            if let Event::Done(r) = e {
                Some(r)
            } else {
                None
            }
        })
        .cloned()
        .or_else(|| w.take_latest().map(Box::new))
        .unwrap();
    assert_eq!(r.sequence, 0);
    assert_eq!(r.curves.len(), 4);
}
#[test]
fn power_sweep_hits_user_defined_output_p1db_and_has_power_axis() {
    let mut g = Graph::network_demo();
    let d = g.nodes.iter_mut().find(|n| n.kind == Kind::Dut).unwrap();
    d.config.loss_db = -20.;
    d.config.limits.output_p1db_dbm = Some(34.);
    let p = g.nodes.iter_mut().find(|n| n.kind == Kind::PnaX).unwrap();
    p.config.instrument.pna.power_sweep = true;
    p.config.instrument.pna.power_start_dbm = 5.;
    p.config.instrument.pna.power_stop_dbm = 25.;
    p.config.points = 21;
    let r = Engine::default()
        .execute(&g, 0, "unused", false, &AtomicBool::new(false))
        .unwrap();
    assert!(r.network.is_none());
    assert_eq!(r.curves[0].x_unit, "dBm");
    assert_eq!(r.curves[0].x[10], 15.);
    assert!((r.curves[0].y[10] - 19.).abs() < 1e-8);
}
