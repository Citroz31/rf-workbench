//! Worker-owned sessions, ordered graph execution and supervised Python IPC.
pub mod python;
mod simulation;
use rf_core::{Graph, Kind, Measurement, NetworkTrace, TestResult, Trace, Waveform};
use rf_instruments::{Resource, ResourceManager, Session};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

pub type Result<T> = std::result::Result<T, String>;
#[derive(Clone, Debug)]
pub struct RunResult {
    pub trace: Option<Trace>,
    pub network: Option<NetworkTrace>,
    pub waveform: Option<Waveform>,
    pub measurements: Vec<Measurement>,
    pub tests: Vec<TestResult>,
    pub completed: Vec<u64>,
    pub elapsed_ms: f64,
    pub sequence: u64,
}
#[derive(Clone)]
pub(crate) enum Value {
    Signal {
        frequency: f64,
        level: f64,
        simulated: bool,
    },
    Trace(Trace),
    Scalar(f64),
    Analog(Waveform),
    Digital(Waveform),
    Dut {
        loss: f64,
        noise: f64,
    },
    Network,
}
#[derive(Default)]
pub struct Engine {
    sessions: BTreeMap<String, Box<dyn Session>>,
    armed: BTreeSet<String>,
}
impl Engine {
    pub fn query(&mut self, resource: &str, command: &str, hardware: bool) -> Result<String> {
        self.session(resource, hardware)?
            .query(command)
            .map_err(|e| e.to_string())
    }
    pub fn write(&mut self, resource: &str, command: &str, hardware: bool) -> Result<()> {
        let enabled = matches!(
            command.trim().to_ascii_uppercase().as_str(),
            ":OUTP ON" | ":OUTP 1"
        );
        // Track before writing: even a partial write may enable RF output.
        if enabled {
            self.armed.insert(resource.into());
        }
        self.session(resource, hardware)?
            .write(command)
            .map_err(|e| e.to_string())
    }
    fn session(&mut self, resource: &str, hardware: bool) -> Result<&mut Box<dyn Session>> {
        let parsed = Resource::parse(resource).map_err(|e| e.to_string())?;
        if parsed != Resource::Sim && !hardware {
            return Err("Mode simulation : activer le matériel dans la barre d'outils".into());
        }
        if !self.sessions.contains_key(resource) {
            let s = ResourceManager
                .open_resource(resource, Duration::from_secs(2))
                .map_err(|e| e.to_string())?;
            self.sessions.insert(resource.into(), s);
        }
        Ok(self.sessions.get_mut(resource).expect("inserted session"))
    }
    /// Best-effort RF shutdown on stop, error and completion of a one-shot run.
    pub fn stop_outputs(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        for name in &self.armed {
            if let Some(session) = self.sessions.get_mut(name)
                && let Err(e) = session.write(":OUTP OFF")
            {
                errors.push(format!("{name}: arrêt RF non confirmé : {e}"));
            }
        }
        self.armed.clear();
        self.sessions.clear();
        errors
    }
    pub fn execute(
        &mut self,
        graph: &Graph,
        sequence: u64,
        python_path: &str,
        hardware: bool,
        cancelled: &AtomicBool,
    ) -> Result<RunResult> {
        graph.validate().map_err(|e| e.to_string())?;
        for n in graph.nodes.iter().filter(|n| n.kind.is_extended()) {
            if Resource::parse(&n.config.resource).map_err(|e| e.to_string())? != Resource::Sim {
                return Err(format!("{} : profil matériel non implémenté", n.title));
            }
        }
        let start = Instant::now();
        let mut values = BTreeMap::new();
        let mut result = RunResult {
            trace: None,
            network: None,
            waveform: None,
            measurements: Vec::new(),
            tests: Vec::new(),
            completed: Vec::new(),
            elapsed_ms: 0.,
            sequence,
        };
        for id in graph.order().map_err(|e| e.to_string())? {
            if cancelled.load(Ordering::Relaxed) {
                return Err("Exécution arrêtée".into());
            }
            let node = graph.node(id).expect("validated node");
            let c = &node.config;
            let inputs: BTreeMap<usize, Value> = graph
                .edges
                .iter()
                .filter(|e| e.to == id)
                .filter_map(|e| {
                    values
                        .get(&(e.from, e.from_port))
                        .cloned()
                        .map(|v| (e.to_port, v))
                })
                .collect();
            if node.kind.is_extended() {
                let output = simulation::execute(node, &inputs, &mut result)?;
                for (port, value) in output.into_iter().enumerate() {
                    values.insert((id, port), value);
                }
                result.completed.push(id);
                continue;
            }
            let input = inputs.get(&0).cloned();
            if node.kind == Kind::Dut {
                values.insert(
                    (id, 1),
                    Value::Dut {
                        loss: c.loss_db,
                        noise: c.noise_figure_db,
                    },
                );
            }
            let value = match (node.kind, input) {
                (Kind::Generator, _) => {
                    self.write(&c.resource, &format!(":FREQ {}", c.frequency_hz), hardware)?;
                    self.write(&c.resource, &format!(":POW {}", c.power_dbm), hardware)?;
                    self.write(&c.resource, ":OUTP ON", hardware)?;
                    Value::Signal {
                        frequency: c.frequency_hz,
                        level: c.power_dbm,
                        simulated: Resource::parse(&c.resource).map_err(|e| e.to_string())?
                            == Resource::Sim,
                    }
                }
                (
                    Kind::Dut,
                    Some(Value::Signal {
                        frequency,
                        level,
                        simulated,
                    }),
                ) => Value::Signal {
                    frequency,
                    level: level - c.loss_db,
                    simulated,
                },
                (
                    Kind::Analyzer,
                    Some(Value::Signal {
                        frequency,
                        level,
                        simulated,
                    }),
                ) => {
                    let t = if Resource::parse(&c.resource).map_err(|e| e.to_string())?
                        == Resource::Sim
                    {
                        if !simulated {
                            return Err("Banc mixte refusé : un analyseur simulé ne mesure pas un générateur réel".into());
                        }
                        rf_instruments::simulate_trace(c, frequency, level, sequence)
                            .map_err(|e| e.to_string())?
                    } else {
                        if simulated {
                            return Err("Banc mixte refusé : connecter un générateur réel à l'analyseur réel".into());
                        }
                        // Generic spectrum analyzer profile. Model-specific drivers can override these commands.
                        for command in [
                            format!(":FREQ:STAR {}", c.start_hz),
                            format!(":FREQ:STOP {}", c.stop_hz),
                            format!(":SWE:POIN {}", c.points),
                            ":FORM ASC".into(),
                            ":INIT:CONT OFF".into(),
                            ":INIT:IMM".into(),
                        ] {
                            self.write(&c.resource, &command, hardware)?;
                        }
                        if self.query(&c.resource, "*OPC?", hardware)?.trim() != "1" {
                            return Err("Acquisition non terminée".into());
                        }
                        let data = self.query(&c.resource, &c.trace_query, hardware)?;
                        let amplitude_dbm: Vec<f64> = data
                            .split(',')
                            .map(|v| {
                                v.trim()
                                    .parse::<f64>()
                                    .map_err(|_| "Trace SCPI ASCII invalide".to_string())
                            })
                            .collect::<Result<_>>()?;
                        if amplitude_dbm.len() != c.points {
                            return Err(format!(
                                "Nombre de points inattendu : {} / {}",
                                amplitude_dbm.len(),
                                c.points
                            ));
                        }
                        Trace {
                            frequency_hz: (0..c.points)
                                .map(|i| {
                                    c.start_hz
                                        + (c.stop_hz - c.start_hz) * i as f64
                                            / (c.points - 1) as f64
                                })
                                .collect(),
                            amplitude_dbm,
                            simulated: false,
                        }
                    };
                    t.validate().map_err(|e| e.to_string())?;
                    result.trace = Some(t.clone());
                    Value::Trace(t)
                }
                (Kind::Dut, None) => Value::Signal {
                    frequency: c.frequency_hz,
                    level: c.power_dbm - c.loss_db,
                    simulated: true,
                },
                (Kind::Python, Some(Value::Trace(t))) => {
                    let transformed = python::run(
                        python_path,
                        &c.script,
                        &t,
                        hardware,
                        cancelled,
                        |resource, cmd, write| {
                            if write {
                                self.write(resource, cmd, hardware).map(|_| String::new())
                            } else {
                                self.query(resource, cmd, hardware)
                            }
                        },
                    )?;
                    result.trace = Some(transformed.clone());
                    Value::Trace(transformed)
                }
                (Kind::Peak, Some(Value::Trace(t))) => {
                    Value::Scalar(t.peak().map_err(|e| e.to_string())?.1)
                }
                (Kind::Limit, Some(Value::Scalar(v))) => {
                    result.tests.push(TestResult {
                        name: node.title.clone(),
                        passed: (c.lower_dbm..=c.upper_dbm).contains(&v),
                        detail: format!(
                            "{v:.3} dBm ; limites [{:.3}, {:.3}] dBm",
                            c.lower_dbm, c.upper_dbm
                        ),
                    });
                    Value::Scalar(v)
                }
                _ => return Err(format!("{} : donnée d'entrée incompatible", node.title)),
            };
            values.insert((id, 0), value);
            result.completed.push(id);
        }
        result.elapsed_ms = start.elapsed().as_secs_f64() * 1000.;
        Ok(result)
    }
}

pub fn measurement_demo() -> Graph {
    let mut g = Graph::default();
    let rf = g.add(Kind::Generator, [30., 30.]);
    let sensor = g.add(Kind::PowerSensor, [320., 30.]);
    let meter = g.add(Kind::PowerMeter, [610., 30.]);
    g.connect(rf, sensor).unwrap();
    g.connect(sensor, meter).unwrap();
    let stream = g.add(Kind::Thermostream, [30., 320.]);
    g.nodes.last_mut().unwrap().config.temperature_c = 85.;
    let thermometer = g.add(Kind::Thermometer, [320., 320.]);
    g.connect(stream, thermometer).unwrap();
    g.add(Kind::VariableResistor, [610., 320.]);
    g.add(Kind::NoiseFigureMeter, [900., 320.]);
    g
}
pub fn self_tests() -> Vec<TestResult> {
    [
        ("Chaîne I/Q multiports", Graph::iq_demo()),
        ("PNA-X / paramètres S", Graph::network_demo()),
        ("Capteurs et unités", measurement_demo()),
    ]
    .into_iter()
    .map(|(name, g)| {
        let r = Engine::default().execute(&g, 0, "unused", false, &AtomicBool::new(false));
        TestResult {
            name: name.into(),
            passed: r.is_ok(),
            detail: match r {
                Ok(_) => "Modèle idéal simulé exécuté selon les ports".into(),
                Err(e) => e,
            },
        }
    })
    .collect()
}

pub enum Command {
    Run {
        graph: Graph,
        continuous: bool,
        python_path: String,
        hardware: bool,
    },
    Suite,
    Query {
        resource: String,
        command: String,
        hardware: bool,
    },
    Python {
        source: String,
        trace: Trace,
        python_path: String,
        hardware: bool,
    },
}
#[derive(Clone, Debug)]
pub enum Event {
    Done(Box<RunResult>),
    Suite(Vec<TestResult>),
    Message(String),
    Error(String),
    Idle,
}

/// Trace results use a single replaceable slot; slow rendering cannot queue old traces.
pub struct Worker {
    tx: mpsc::SyncSender<Command>,
    pub events: mpsc::Receiver<Event>,
    latest: Arc<Mutex<Option<RunResult>>>,
    cancel: Arc<AtomicBool>,
    busy: Arc<AtomicBool>,
}
impl Worker {
    pub fn spawn(wake: impl Fn() + Send + 'static) -> Self {
        let (tx, rx) = mpsc::sync_channel(8);
        let (etx, events) = mpsc::sync_channel(64);
        let latest = Arc::new(Mutex::new(None));
        let cancel = Arc::new(AtomicBool::new(false));
        let busy = Arc::new(AtomicBool::new(false));
        let (c, l, b) = (cancel.clone(), latest.clone(), busy.clone());
        std::thread::spawn(move || {
            let mut engine = Engine::default();
            let mut sequence = 0;
            while let Ok(command) = rx.recv() {
                // Submit sets busy before enqueue. Cancellation isn't reset here:
                // an immediate Stop must also cancel a queued command.
                let mut shutdown = true;
                match command {
                    Command::Run {
                        graph,
                        continuous,
                        python_path,
                        hardware,
                    } => loop {
                        match engine.execute(&graph, sequence, &python_path, hardware, &c) {
                            Ok(result) => {
                                sequence += 1;
                                if let Ok(mut slot) = l.lock() {
                                    *slot = Some(result);
                                }
                                wake();
                            }
                            Err(e) => {
                                let _ = etx.try_send(Event::Error(e));
                                break;
                            }
                        }
                        if !continuous || c.load(Ordering::Relaxed) {
                            break;
                        }
                        // 10 ms polling bounds cancellation between acquisitions.
                        for _ in 0..25 {
                            if c.load(Ordering::Relaxed) {
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(10));
                        }
                    },
                    Command::Suite => {
                        let mut tests = rf_core::self_tests();
                        tests.extend(rf_instruments::self_tests());
                        tests.extend(self_tests());
                        let _ = etx.try_send(Event::Suite(tests));
                    }
                    Command::Query {
                        resource,
                        command,
                        hardware,
                    } => {
                        let response = engine.query(&resource, &command, hardware);
                        shutdown = response.is_err();
                        let _ = etx.try_send(match response {
                            Ok(s) => Event::Message(format!("{command} → {s}")),
                            Err(e) => Event::Error(e),
                        });
                    }
                    Command::Python {
                        source,
                        trace,
                        python_path,
                        hardware,
                    } => {
                        match python::run(
                            &python_path,
                            &source,
                            &trace,
                            hardware,
                            &c,
                            |r, cmd, write| {
                                if write {
                                    engine.write(r, cmd, hardware).map(|_| String::new())
                                } else {
                                    engine.query(r, cmd, hardware)
                                }
                            },
                        ) {
                            Ok(trace) => {
                                let _ = etx.try_send(Event::Done(Box::new(RunResult {
                                    trace: Some(trace),
                                    network: None,
                                    waveform: None,
                                    measurements: Vec::new(),
                                    tests: Vec::new(),
                                    completed: Vec::new(),
                                    elapsed_ms: 0.,
                                    sequence,
                                })));
                            }
                            Err(e) => {
                                let _ = etx.try_send(Event::Error(e));
                            }
                        }
                    }
                }
                if shutdown {
                    for error in engine.stop_outputs() {
                        let _ = etx.try_send(Event::Error(error));
                    }
                }
                b.store(false, Ordering::Release);
                let _ = etx.try_send(Event::Idle);
                wake();
            }
            engine.stop_outputs();
        });
        Self {
            tx,
            events,
            latest,
            cancel,
            busy,
        }
    }
    pub fn submit(&self, command: Command) -> Result<()> {
        if self
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err("Une tâche est déjà en cours".into());
        }
        self.cancel.store(false, Ordering::Relaxed);
        if let Err(e) = self.tx.try_send(command) {
            self.busy.store(false, Ordering::Release);
            return Err(e.to_string());
        }
        Ok(())
    }
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }
    pub fn take_latest(&self) -> Option<RunResult> {
        self.latest.lock().ok()?.take()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_graph_never_opens_hardware() {
        let mut g = Graph::demo();
        g.edges.clear();
        assert!(
            Engine::default()
                .execute(&g, 0, "python", false, &AtomicBool::new(false))
                .is_err()
        );
    }
    #[test]
    fn hardware_requires_explicit_mode() {
        assert!(
            Engine::default()
                .query("TCPIP::127.0.0.1::5025::SOCKET", "*IDN?", false)
                .unwrap_err()
                .contains("Mode simulation")
        );
    }
    #[test]
    fn run_without_python_evaluates_real_connections() {
        let mut g = Graph::demo();
        g.remove(4);
        g.connect(3, 5).unwrap();
        let mut engine = Engine::default();
        let r = engine
            .execute(&g, 0, "unused", false, &AtomicBool::new(false))
            .unwrap();
        assert_eq!(r.completed, vec![1, 2, 3, 5, 6]);
        assert!(r.tests[0].passed);
        assert_eq!(r.trace.unwrap().peak().unwrap(), (2.45e9, -13.));
        g.nodes
            .iter_mut()
            .find(|n| n.kind == Kind::Dut)
            .unwrap()
            .config
            .loss_db = 20.;
        assert!(
            !engine
                .execute(&g, 1, "unused", false, &AtomicBool::new(false))
                .unwrap()
                .tests[0]
                .passed
        );
    }
    #[test]
    fn cancellation_prevents_commands() {
        assert!(
            Engine::default()
                .execute(&Graph::demo(), 0, "unused", false, &AtomicBool::new(true))
                .unwrap_err()
                .contains("arrêtée")
        );
    }
}
