use rf_core::dsp::{Data, Op, Unit};
use rf_core::{Graph, Kind};
use rf_runtime::{Engine, debug::BufferData};
use std::sync::atomic::AtomicBool;
#[test]
fn dsp_demo_uses_typed_references_and_has_zero_ber() {
    let graph = rf_runtime::dsp_demo();
    graph.validate().unwrap();
    let result = Engine::default()
        .execute(&graph, 0, "unused", false, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(result.completed.len(), 11);
    assert_eq!(result.measurements.len(), 4);
    assert_eq!(
        result
            .measurements
            .iter()
            .find(|m| m.name == "BER")
            .unwrap()
            .value,
        0.
    );
    assert!(
        (result
            .measurements
            .iter()
            .find(|m| m.name == "SNR")
            .unwrap()
            .value
            - 30.)
            .abs()
            < 0.5
    );
    assert!(
        result
            .buffers
            .iter()
            .any(|b| matches!(b.data.as_ref(), BufferData::Dsp(Data::Bits(_))))
    );
    assert!(
        result
            .buffers
            .iter()
            .any(|b| matches!(b.data.as_ref(), BufferData::Dsp(Data::Iq(_))))
    );
    assert!(rf_runtime::self_tests().iter().all(|t| t.passed));
}
#[test]
fn vswr_uses_its_own_vna_not_the_last_acquisition() {
    let mut g = Graph::default();
    let first = g.add(Kind::Pna, [0., 0.]);
    g.nodes.last_mut().unwrap().config.s_parameter = "S11".into();
    let second = g.add(Kind::PnaX, [0., 300.]);
    g.nodes.last_mut().unwrap().config.s_parameter = "S21".into();
    let vswr = g.add(Kind::Dsp(Op::Vswr), [300., 0.]);
    g.connect(first, vswr).unwrap();
    let result = Engine::default()
        .execute(&g, 0, "unused", false, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(result.network.unwrap().parameter, "S21");
    let measured = result
        .measurements
        .iter()
        .find(|m| m.name == "VSWR")
        .unwrap();
    assert!(measured.value > 1. && measured.value < 1.3);
    g.edges.clear();
    g.connect(second, vswr).unwrap();
    assert!(
        Engine::default()
            .execute(&g, 0, "unused", false, &AtomicBool::new(false))
            .unwrap_err()
            .contains("VSWR")
    );
}
#[test]
fn every_dsp_block_serializes_with_parameters_and_ports() {
    for op in Op::ALL {
        let mut g = Graph::default();
        g.add(Kind::Dsp(op), [0., 0.]);
        let p = rf_core::Project {
            graph: g,
            ..Default::default()
        };
        let json = p.to_json().unwrap();
        let restored = rf_core::Project::from_json(&json).unwrap();
        assert_eq!(restored.graph.nodes[0].kind, Kind::Dsp(op));
        assert!(!op.outputs().is_empty());
        assert_eq!(restored.graph.nodes[0].config.dsp.target_unit, Unit::Dbm);
    }
}
#[test]
fn physical_io_is_refused_before_any_session() {
    let mut g = Graph::default();
    g.add(Kind::Dsp(Op::IoSource), [0., 0.]);
    g.nodes[0].config.dsp.io_backend = "VISA".into();
    g.nodes[0].config.dsp.endpoint = "GPIB0::16::INSTR".into();
    assert!(
        Engine::default()
            .execute(&g, 0, "unused", false, &AtomicBool::new(false))
            .unwrap_err()
            .contains("activer le matériel")
    );
}
