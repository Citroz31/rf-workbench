#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod analysis;
mod app;
mod bench_file;
mod canvas;
mod editor;
mod file_browser;
mod help;
mod i18n;
mod plot;
#[cfg(test)]
mod plot_tests;
mod shortcuts;
mod studio;
mod theme;
mod visuals;

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
            serde_json::json!({"iterations":iterations,"total_ms":start.elapsed().as_secs_f64()*1000.,"last_run_ms":last.elapsed_ms,"peak":last.trace.and_then(|t|t.peak().ok()),"measurements":last.measurements.iter().map(|m|serde_json::json!({"name":m.name,"value":m.value,"unit":m.unit,"simulated":m.simulated})).collect::<Vec<_>>(),"tests":last.tests,"completed":last.completed})
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
