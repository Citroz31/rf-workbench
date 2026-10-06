//! Run with: cargo test -p rf-runtime --test python_integration -- --ignored
use rf_core::Config;
use rf_runtime::{Engine, python};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

fn interpreter() -> String {
    std::env::var("RF_WORKBENCH_TEST_PYTHON").unwrap_or_else(|_| "python".into())
}
fn trace() -> rf_core::Trace {
    rf_instruments::simulate_trace(&Config::default(), 2.45e9, -13., 0).unwrap()
}

#[test]
#[ignore = "requires a Python 3.10+ interpreter"]
fn real_python_process_delegates_scpi_and_compensates_trace() {
    let mut engine = Engine::default();
    let script = "rm = ResourceManager()\ni = rm.open_resource('SIM::RF::INSTR')\ni.write(':POW -5')\nassert i.query(':POW?') == '-5'\noutput = dict(trace)\noutput['amplitude_dbm'] = [x + 0.5 for x in trace['amplitude_dbm']]\n";
    let t = python::run(
        &interpreter(),
        script,
        &trace(),
        false,
        &AtomicBool::new(false),
        |r, c, w| {
            if w {
                engine.write(r, c, false).map(|_| String::new())
            } else {
                engine.query(r, c, false)
            }
        },
    )
    .unwrap();
    assert_eq!(t.peak().unwrap().1, -12.5);
}
#[test]
#[ignore = "requires a Python 3.10+ interpreter"]
fn invalid_python_output_is_rejected_and_simulation_provenance_preserved() {
    let cancel = AtomicBool::new(false);
    let malformed = python::run(
        &interpreter(),
        "output = dict(trace)\noutput['frequency_hz'] = [1]",
        &trace(),
        false,
        &cancel,
        |_, _, _| Ok(String::new()),
    );
    assert!(malformed.unwrap_err().contains("Résultat Python"));
    let t = python::run(
        &interpreter(),
        "output = dict(trace)\noutput['simulated'] = False",
        &trace(),
        false,
        &cancel,
        |_, _, _| Ok(String::new()),
    )
    .unwrap();
    assert!(t.simulated);
}
#[test]
#[ignore = "requires a Python 3.10+ interpreter"]
fn python_traceback_and_hardware_mode_errors_are_visible() {
    let cancel = AtomicBool::new(false);
    let mut engine = Engine::default();
    let err = python::run(
        &interpreter(),
        "ResourceManager().open_resource('TCPIP::127.0.0.1::5025::SOCKET').query('*IDN?')",
        &trace(),
        false,
        &cancel,
        |r, c, _| engine.query(r, c, false),
    )
    .unwrap_err();
    assert!(err.contains("Mode simulation"));
    assert!(err.contains("RuntimeError"));
}
#[test]
#[ignore = "requires a Python 3.10+ interpreter"]
fn stop_terminates_an_infinite_python_loop() {
    let cancel = Arc::new(AtomicBool::new(false));
    let c = cancel.clone();
    let stopper = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        c.store(true, Ordering::Relaxed);
    });
    let start = Instant::now();
    let err = python::run(
        &interpreter(),
        "while True: pass",
        &trace(),
        false,
        &cancel,
        |_, _, _| Ok(String::new()),
    )
    .unwrap_err();
    assert!(err.contains("arrêté"));
    assert!(start.elapsed() < Duration::from_secs(3));
    stopper.join().unwrap();
}
#[test]
#[ignore = "requires a Python 3.10+ interpreter"]
fn timeout_terminates_a_script_without_stop() {
    let start = Instant::now();
    let err = python::run(
        &interpreter(),
        "while True: pass",
        &trace(),
        false,
        &AtomicBool::new(false),
        |_, _, _| Ok(String::new()),
    )
    .unwrap_err();
    assert!(err.contains("5 s dépassé"));
    assert!(start.elapsed() < Duration::from_secs(8));
}
