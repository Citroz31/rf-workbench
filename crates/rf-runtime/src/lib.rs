//! Worker-owned sessions, ordered graph execution and supervised Python IPC.
pub mod debug;
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
    pub buffers: Vec<debug::Buffer>,
    pub trace: Option<Trace>,
    pub network: Option<NetworkTrace>,
    pub waveform: Option<Waveform>,
    pub measurements: Vec<Measurement>,
    pub tests: Vec<TestResult>,
    pub completed: Vec<u64>,
    pub elapsed_ms: f64,
    pub sequence: u64,
    pub overflows: usize,
}
#[derive(Clone)]
pub(crate) enum Value {
    Signal {
        frequency: f64,
        level: f64,
        simulated: bool,
    },
    Trace(Trace),
    Scalar(f64, bool),
    Analog(Waveform),
    Digital(Waveform),
    Dut {
        loss: f64,
        noise: f64,
    },
    Network(NetworkTrace),
    Dsp(Arc<rf_core::dsp::Data>),
}
#[derive(Default)]
pub struct Engine {
    sessions: BTreeMap<String, Box<dyn Session>>,
    armed: BTreeSet<String>,
    dsp: rf_dsp::Processor,
    io: BTreeMap<u64, rf_hal::transport::Io>,
    sources: BTreeMap<u64, rf_hal::streaming::Source>,
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
        self.io.clear();
        self.sources.clear();
        self.dsp.reset();
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
        self.execute_observed(
            graph,
            sequence,
            python_path,
            hardware,
            cancelled,
            |_, _, _| Ok(()),
        )
    }
    pub fn execute_observed(
        &mut self,
        graph: &Graph,
        sequence: u64,
        python_path: &str,
        hardware: bool,
        cancelled: &AtomicBool,
        mut observe: impl FnMut(debug::Phase, &rf_core::Node, &RunResult) -> Result<()>,
    ) -> Result<RunResult> {
        graph.validate().map_err(|e| e.to_string())?;
        if sequence == 0 {
            self.dsp.reset();
            self.io.clear();
            self.sources.clear();
        }
        for n in &graph.nodes {
            if let Kind::Dsp(op) = n.kind
                && matches!(op, rf_core::dsp::Op::IoSource | rf_core::dsp::Op::IoSink)
                && !matches!(n.config.dsp.io_backend.as_str(), "RAW" | "HDF5" | "PARQUET")
                && !hardware
            {
                return Err(format!(
                    "{} : activer le matériel pour ce backend HAL",
                    n.title
                ));
            }
        }
        for n in graph.nodes.iter().filter(|n| n.kind.is_extended()) {
            if Resource::parse(&n.config.resource).map_err(|e| e.to_string())? != Resource::Sim {
                return Err(format!("{} : profil matériel non implémenté", n.title));
            }
        }
        let start = Instant::now();
        let mut values = BTreeMap::new();
        let mut result = RunResult {
            buffers: Vec::new(),
            trace: None,
            network: None,
            waveform: None,
            measurements: Vec::new(),
            tests: Vec::new(),
            completed: Vec::new(),
            elapsed_ms: 0.,
            sequence,
            overflows: 0,
        };
        for id in graph.order().map_err(|e| e.to_string())? {
            if cancelled.load(Ordering::Relaxed) {
                return Err("Exécution arrêtée".into());
            }
            let node = graph.node(id).expect("validated node");
            observe(debug::Phase::Before, node, &result)?;
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
            if let Kind::Dsp(op) = node.kind {
                use rf_core::dsp::{Data, Op, Unit};
                let data = if matches!(op, Op::IoSource | Op::IoSink) {
                    let mut settings = c.dsp.clone();
                    if settings.python_executable == "python" {
                        settings.python_executable = python_path.into();
                    }
                    if op == Op::IoSource {
                        if let std::collections::btree_map::Entry::Vacant(entry) =
                            self.sources.entry(id)
                        {
                            entry.insert(rf_hal::streaming::Source::open(&settings)?);
                        }
                        let source = self.sources.get_mut(&id).expect("opened source");
                        let frame = source.read()?;
                        result.overflows += source.overflows();
                        Data::Iq(frame)
                    } else {
                        if let std::collections::btree_map::Entry::Vacant(entry) = self.io.entry(id)
                        {
                            entry.insert(rf_hal::transport::Io::open(&settings, false)?);
                        }
                        let io = self.io.get_mut(&id).expect("opened sink");
                        let Some(Value::Dsp(d)) = inputs.get(&0) else {
                            return Err("Sortie HAL : I/Q requis".into());
                        };
                        let Data::Iq(f) = d.as_ref() else {
                            return Err("Sortie HAL : I/Q requis".into());
                        };
                        io.write(f)?;
                        Data::Iq(f.clone())
                    }
                } else if op == Op::Vswr {
                    let Some(Value::Network(t)) = inputs.get(&0) else {
                        return Err("VSWR : réflexion S11/S22 requise".into());
                    };
                    if !matches!(t.parameter.as_str(), "S11" | "S22") {
                        return Err("VSWR ne s'applique pas à S21/S12".into());
                    }
                    let rho = t
                        .magnitude_db
                        .iter()
                        .map(|x| 10f64.powf(x / 20.))
                        .fold(0., f64::max);
                    if rho >= 1. {
                        return Err("VSWR indéfini : |réflexion| >= 1".into());
                    }
                    Data::Quantity {
                        value: (1. + rho) / (1. - rho),
                        unit: Unit::Ratio,
                        simulated: t.simulated,
                    }
                } else {
                    let data: Vec<Data> = inputs
                        .values()
                        .map(|v| match v {
                            Value::Dsp(d) => Ok(d.as_ref().clone()),
                            _ => Err("Port DSP incompatible".to_string()),
                        })
                        .collect::<Result<_>>()?;
                    self.dsp.process(id, op, &c.dsp, &data, sequence)?
                };
                data.validate()?;
                if let Data::Quantity {
                    value,
                    unit,
                    simulated,
                } = &data
                {
                    result.measurements.push(Measurement {
                        name: node.title.clone(),
                        value: *value,
                        unit: unit.label().into(),
                        simulated: *simulated,
                    });
                }
                values.insert((id, 0), Value::Dsp(Arc::new(data)));
                result.completed.push(id);
                capture(&mut result, node, &values);
                observe(debug::Phase::After, node, &result)?;
                continue;
            }
            if node.kind.is_extended() {
                let output = simulation::execute(node, &inputs, &mut result)?;
                for (port, value) in output.into_iter().enumerate() {
                    values.insert((id, port), value);
                }
                result.completed.push(id);
                capture(&mut result, node, &values);
                observe(debug::Phase::After, node, &result)?;
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
                    Value::Scalar(t.peak().map_err(|e| e.to_string())?.1, t.simulated)
                }
                (Kind::Limit, Some(Value::Scalar(v, simulated))) => {
                    result.tests.push(TestResult {
                        name: node.title.clone(),
                        passed: (c.lower_dbm..=c.upper_dbm).contains(&v),
                        detail: format!(
                            "{v:.3} dBm ; limites [{:.3}, {:.3}] dBm",
                            c.lower_dbm, c.upper_dbm
                        ),
                    });
                    Value::Scalar(v, simulated)
                }
                _ => return Err(format!("{} : donnée d'entrée incompatible", node.title)),
            };
            values.insert((id, 0), value);
            result.completed.push(id);
            capture(&mut result, node, &values);
            observe(debug::Phase::After, node, &result)?;
        }
        result.elapsed_ms = start.elapsed().as_secs_f64() * 1000.;
        Ok(result)
    }
}

fn capture(result: &mut RunResult, node: &rf_core::Node, values: &BTreeMap<(u64, usize), Value>) {
    for port in 0..node.kind.outputs().len() {
        if let Some(v) = values.get(&(node.id, port)) {
            let buffer = debug::Buffer::capture(node, port, v, result);
            if result.buffers.len() >= 512 {
                if let Some(i) = result.buffers.iter().position(|b| !b.probe) {
                    result.buffers.remove(i);
                } else {
                    continue;
                }
            }
            result.buffers.push(buffer);
        }
    }
}

pub fn dsp_demo() -> Graph {
    use rf_core::dsp::Op;
    let mut g = Graph::default();
    for (op, pos) in [
        (Op::BitSource, [20., 30.]),
        (Op::ConvEncode, [310., 30.]),
        (Op::DigitalMod, [600., 30.]),
        (Op::Channel, [890., 30.]),
        (Op::DigitalDemod, [1180., 30.]),
        (Op::ConvDecode, [1470., 30.]),
        (Op::Ber, [1760., 30.]),
        (Op::Psd, [890., 310.]),
        (Op::Evm, [1180., 310.]),
        (Op::Snr, [1470., 310.]),
        (Op::Power, [600., 310.]),
    ] {
        g.add(Kind::Dsp(op), pos);
    }
    for (f, fp, t, tp) in [
        (1, 0, 2, 0),
        (2, 0, 3, 0),
        (3, 0, 4, 0),
        (4, 0, 5, 0),
        (5, 0, 6, 0),
        (6, 0, 7, 0),
        (1, 0, 7, 1),
        (4, 0, 8, 0),
        (4, 0, 9, 0),
        (3, 0, 9, 1),
        (4, 0, 10, 0),
        (3, 0, 10, 1),
        (4, 0, 11, 0),
    ] {
        g.connect_ports(f, fp, t, tp).expect("DSP demo");
    }
    g
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
        ("QAM16 / AWGN / Viterbi / BER", dsp_demo()),
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
    Discover,
    Debug {
        graph: Graph,
        python_path: String,
    },
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
    Resources(Vec<String>),
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
    debug_latest: Arc<Mutex<Option<debug::Snapshot>>>,
    debug_control: Arc<std::sync::atomic::AtomicU8>,
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
        let debug_latest = Arc::new(Mutex::new(None));
        let debug_control = Arc::new(std::sync::atomic::AtomicU8::new(0));
        let (debug_slot, control) = (debug_latest.clone(), debug_control.clone());
        let (c, l, b) = (cancel.clone(), latest.clone(), busy.clone());
        std::thread::spawn(move || {
            let mut engine = Engine::default();
            let mut sequence = 0;
            while let Ok(command) = rx.recv() {
                // Submit sets busy before enqueue. Cancellation isn't reset here:
                // an immediate Stop must also cancel a queued command.
                let mut shutdown = true;
                match command {
                    Command::Discover => match rf_hal::discover_visa() {
                        Ok(v) => {
                            let _ = etx.send(Event::Resources(v));
                        }
                        Err(e) => {
                            let _ = etx.send(Event::Error(e));
                        }
                    },
                    Command::Debug { graph, python_path } => {
                        let mut single_step = true;
                        let valid = graph.nodes.iter().all(|n| {
                            Resource::parse(&n.config.resource).ok() == Some(Resource::Sim)
                        });
                        let result = if !valid {
                            Err("Le débogage pas à pas est réservé à un banc simulé".into())
                        } else {
                            engine.execute_observed(
                                &graph,
                                sequence,
                                &python_path,
                                false,
                                &c,
                                |phase, node, result| {
                                    let paused = phase == debug::Phase::Before
                                        && (single_step || node.breakpoint);
                                    if let Ok(mut slot) = debug_slot.lock() {
                                        *slot = Some(debug::Snapshot {
                                            node: node.id,
                                            title: node.title.clone(),
                                            paused,
                                            completed: result.completed.clone(),
                                            buffers: result.buffers.clone(),
                                        });
                                    }
                                    wake();
                                    if paused {
                                        loop {
                                            if c.load(Ordering::Acquire) {
                                                return Err("Débogage arrêté".into());
                                            }
                                            match control.swap(0, Ordering::AcqRel) {
                                                1 => {
                                                    single_step = false;
                                                    break;
                                                }
                                                2 => {
                                                    single_step = true;
                                                    break;
                                                }
                                                _ => std::thread::sleep(Duration::from_millis(10)),
                                            }
                                        }
                                    }
                                    Ok(())
                                },
                            )
                        };
                        match result {
                            Ok(result) => {
                                sequence += 1;
                                if let Ok(mut slot) = l.lock() {
                                    *slot = Some(result);
                                }
                            }
                            Err(e) => {
                                let _ = etx.try_send(Event::Error(e));
                            }
                        }
                    }
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
                        tests.extend(rf_dsp::self_tests());
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
                                    buffers: Vec::new(),
                                    network: None,
                                    waveform: None,
                                    measurements: Vec::new(),
                                    tests: Vec::new(),
                                    completed: Vec::new(),
                                    elapsed_ms: 0.,
                                    sequence,
                                    overflows: 0,
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
            debug_latest,
            debug_control,
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
        self.debug_control.store(0, Ordering::Release);
        if let Ok(mut slot) = self.debug_latest.lock() {
            *slot = None;
        }
        if let Err(e) = self.tx.try_send(command) {
            self.busy.store(false, Ordering::Release);
            return Err(e.to_string());
        }
        Ok(())
    }
    pub fn debug_continue(&self) {
        self.debug_control.store(1, Ordering::Release);
    }
    pub fn debug_step(&self) {
        self.debug_control.store(2, Ordering::Release);
    }
    pub fn take_debug(&self) -> Option<debug::Snapshot> {
        self.debug_latest.lock().ok()?.take()
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
