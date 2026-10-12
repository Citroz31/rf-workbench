#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod analysis;
mod app;
mod bench_file;
mod canvas;
mod editor;
mod file_browser;
mod frequency_edit;
mod help;
mod i18n;
mod new_project;
mod paths;
mod plot;
#[cfg(test)]
mod plot_tests;
mod project_registry;
mod results;
mod shortcuts;
mod studio;
mod theme;
mod visuals;
mod vna_panel;
mod pna_controls;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--write-examples") {
        let dir = std::path::Path::new(args.get(i + 1).expect("destination examples"));
        std::fs::create_dir_all(dir).expect("examples directory");
        for (file, graph, name) in [
            (
                "PA-36-38GHz.rfbench",
                rf_core::Graph::pa_demo(),
                "PA 36–38 GHz · gain 20 dB (simulation)",
            ),
            (
                "PNA-X.rfbench",
                rf_core::Graph::network_demo(),
                "PNA-X / DUT",
            ),
            (
                "QAM16-AWGN.rfbench",
                rf_runtime::dsp_demo(),
                "QAM16 / AWGN / Viterbi",
            ),
            ("LNA-MAAL-FR1245-Courbes.rfbench",catalog_graph("macom-maal-fr1245"),"MAAL-FR1245 · courbes publiées +25 °C"),
            ("Corechip-CGY2170-Courbes.rfbench",catalog_graph("macom-cgy2170yhv-c1"),"CGY2170YHV/C1 · modules publiés"),
            ("PNA-X-Puissance-Recepteurs.rfbench",power_graph(),"PA · gain / réflexion / R1 / B"),
        ] {
            let b = bench_file::BenchFile::new(
                rf_core::Project {
                    schema_version: rf_core::SCHEMA_VERSION,
                    name: name.into(),
                    graph,
                },
                studio::Layout::default(),
            );
            std::fs::write(dir.join(file), b.json().unwrap()).unwrap();
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--self-test") {
        let mut tests = rf_core::self_tests();
        tests.extend(rf_instruments::self_tests());
        tests.extend(rf_dsp::self_tests());
        tests.extend(rf_runtime::self_tests());
        let passed = tests.iter().all(|t| t.passed);
        println!("{}", serde_json::to_string_pretty(&tests).unwrap());
        std::process::exit(if passed { 0 } else { 1 });
    }
    if args
        .iter()
        .any(|a| a == "--headless-run" || a == "--benchmark")
    {
        let mut graph = rf_core::Graph::demo();
        graph.remove(4);
        graph.connect(3, 5).unwrap();
        if args.iter().any(|a| a == "--demo-dsp") {
            graph = rf_runtime::dsp_demo();
        }
        if let Some(i) = args.iter().position(|a| a == "--open") {
            let text = std::fs::read_to_string(args.get(i + 1).expect("--open path"))
                .expect("read project");
            graph = bench_file::BenchFile::parse(&text)
                .expect("valid simulated project")
                .project
                .graph;
        }
        let iterations = if args.iter().any(|a| a == "--benchmark") {
            1000
        } else {
            1
        };
        let mut engine = rf_runtime::Engine::default();
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let start = std::time::Instant::now();
        let mut last = None;
        for i in 0..iterations {
            last = Some(
                engine
                    .execute(&graph, i, "unused", false, &cancel)
                    .expect("simulated bench"),
            );
        }
        engine.stop_outputs();
        let last = last.unwrap();
        println!(
            "{}",
            serde_json::json!({"iterations":iterations,"total_ms":start.elapsed().as_secs_f64()*1000.,"last_run_ms":last.elapsed_ms,"peak":last.trace.and_then(|t|t.peak().ok()),"network":last.network.map(|n|serde_json::json!({"parameter":n.parameter,"points":n.frequency_hz.len(),"first_magnitude_db":n.magnitude_db.first(),"first_phase_deg":n.phase_deg.first(),"simulated":n.simulated})),"measurements":last.measurements.iter().map(|m|serde_json::json!({"name":m.name,"value":m.value,"unit":m.unit,"simulated":m.simulated})).collect::<Vec<_>>(),"curves":last.curves,"tests":last.tests,"completed":last.completed})
        );
        return Ok(());
    }
    if args.iter().any(|a| a == "--python-smoke") {
        let path = args
            .iter()
            .position(|a| a == "--python")
            .and_then(|i| args.get(i + 1))
            .map(String::as_str)
            .unwrap_or("python");
        let trace =
            rf_instruments::simulate_trace(&rf_core::Config::default(), 2.45e9, -13., 0).unwrap();
        let mut engine = rf_runtime::Engine::default();
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let script = "rm = ResourceManager()\ninstrument = rm.open_resource('SIM::RF::INSTR')\nassert 'Simulator' in instrument.query('*IDN?')\noutput = dict(trace)\noutput['amplitude_dbm'] = [x + 0.5 for x in trace['amplitude_dbm']]\n";
        let result = rf_runtime::python::run(path, script, &trace, false, &cancel, |r, c, w| {
            if w {
                engine.write(r, c, false).map(|_| String::new())
            } else {
                engine.query(r, c, false)
            }
        });
        match result {
            Ok(t) => {
                assert_eq!(t.peak().unwrap().1, -12.5);
                println!("Python IPC + SCPI + compensation : PASS");
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1600., 1000.])
            .with_min_inner_size([1050., 760.]),
        vsync: true,
        ..Default::default()
    };
    eframe::run_native(
        "RF Workbench · Instrumentation & métrologie",
        options,
        Box::new(|cc| Ok(Box::new(app::Workbench::new(cc)))),
    )
}
fn catalog_graph(id:&str)->rf_core::Graph {
    let catalog=rf_dut_library::Catalog::default();let component=catalog.entries.iter().find(|c|c.id==id).unwrap();let rf=component.rf.as_ref().unwrap();
    let mut graph=rf_core::Graph::network_demo();
    for n in &mut graph.nodes {
        if n.kind==rf_core::Kind::Dut {
            n.title=component.part_number.clone();n.config.dut_id=Some(id.into());n.config.dut.dataset=component.dataset.clone();n.config.dut.use_dataset=true;
            n.config.dut.product_type=if id.contains("maal"){rf_core::dut::ProductType::Lna}else{rf_core::dut::ProductType::Corechip};
            n.config.loss_db=-rf.gain_db.typical;n.config.noise_figure_db=rf.noise_figure_db.typical;
            n.config.limits.band_hz=Some(rf.band_hz);n.config.limits.output_p1db_dbm=Some(rf.output_p1db_dbm.typical);
        }else if n.kind.max_rf_ports().is_some(){
            n.config.start_hz=rf.band_hz[0];n.config.stop_hz=rf.band_hz[1];n.config.frequency_hz=rf.reference_frequency_hz;
            n.config.instrument.pna.plan=Some(rf_core::pna_plan::Plan::default());n.config.power_dbm=-35.;
        }
    }
    graph
}
fn power_graph()->rf_core::Graph {
    let mut graph=rf_core::Graph::pa_demo();
    let node=graph.nodes.iter_mut().find(|n|n.kind.max_rf_ports().is_some()).unwrap();
    node.config.instrument.pna.power_sweep=true;node.config.instrument.pna.plan=Some(rf_core::pna_plan::Plan::power(1,2));
    node.config.instrument.pna.power_start_dbm=5.;node.config.instrument.pna.power_stop_dbm=25.;node.config.points=101;
    graph
}
