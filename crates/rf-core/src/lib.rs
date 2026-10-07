//! Platform-independent RF data, typed acyclic graphs and versioned bench files.
pub mod dsp;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_POINTS: usize = 100_001;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Trace {
    pub frequency_hz: Vec<f64>,
    pub amplitude_dbm: Vec<f64>,
    pub simulated: bool,
}
impl Trace {
    pub fn validate(&self) -> Result<()> {
        let n = self.frequency_hz.len();
        if !(2..=MAX_POINTS).contains(&n) || n != self.amplitude_dbm.len() {
            return Err(Error::Invalid(
                "Trace: longueurs invalides (2..100001 points)".into(),
            ));
        }
        if self
            .frequency_hz
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.0)
            || self.amplitude_dbm.iter().any(|v| !v.is_finite())
            || self.frequency_hz.windows(2).any(|w| w[1] <= w[0])
        {
            return Err(Error::Invalid(
                "Trace: valeurs non finies ou fréquences non croissantes".into(),
            ));
        }
        Ok(())
    }
    pub fn peak(&self) -> Result<(f64, f64)> {
        self.validate()?;
        let (index, level) = self
            .amplitude_dbm
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .expect("validated length");
        Ok((self.frequency_hz[index], *level))
    }
    pub fn csv(&self) -> Result<String> {
        self.validate()?;
        let mut csv = format!(
            "# simulated={}\nfrequency_hz,amplitude_dbm\n",
            self.simulated
        );
        for (f, a) in self.frequency_hz.iter().zip(&self.amplitude_dbm) {
            csv.push_str(&format!("{f:.6},{a:.6}\n"));
        }
        Ok(csv)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Port {
    Signal,
    Trace,
    Scalar,
    Analog,
    Digital,
    DutModel,
    SParameters,
    Temperature,
    Resistance,
    NoiseFigure,
    ComplexIq,
    Bits,
    Spectrum,
    Quantity,
}
impl Port {
    pub fn label(self) -> &'static str {
        match self {
            Self::ComplexIq => "I/Q",
            Self::Bits => "bits/LLR",
            Self::Spectrum => "PSD/FFT",
            Self::Quantity => "valeur/unité",
            Self::Signal => "RF",
            Self::Trace => "Trace",
            Self::Scalar => "dBm",
            Self::Analog => "V",
            Self::Digital => "Échantillons",
            Self::DutModel => "Modèle DUT",
            Self::SParameters => "Paramètres S",
            Self::Temperature => "°C",
            Self::Resistance => "Ω",
            Self::NoiseFigure => "NF dB",
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Terminal {
    pub name: &'static str,
    pub port: Port,
    pub required: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Kind {
    Generator,
    Dut,
    Analyzer,
    Python,
    Peak,
    Limit,
    IqModulator,
    Dac,
    Adc,
    Pna,
    PnaX,
    VariableResistor,
    Thermometer,
    Awg,
    NoiseFigureMeter,
    PowerMeter,
    PowerSensor,
    Thermostream,
    Dsp(dsp::Op),
}
impl Kind {
    pub const BASE: [Self; 18] = [
        Self::Generator,
        Self::Analyzer,
        Self::Pna,
        Self::PnaX,
        Self::Awg,
        Self::NoiseFigureMeter,
        Self::PowerMeter,
        Self::PowerSensor,
        Self::IqModulator,
        Self::Dac,
        Self::Adc,
        Self::VariableResistor,
        Self::Thermometer,
        Self::Thermostream,
        Self::Dut,
        Self::Python,
        Self::Peak,
        Self::Limit,
    ];
    pub const ALL: [Self; 18 + dsp::Op::ALL.len()] = {
        let mut a = [Self::Generator; 18 + dsp::Op::ALL.len()];
        let mut i = 0;
        while i < 18 {
            a[i] = Self::BASE[i];
            i += 1;
        }
        let mut j = 0;
        while j < dsp::Op::ALL.len() {
            a[18 + j] = Self::Dsp(dsp::Op::ALL[j]);
            j += 1;
        }
        a
    };
    pub fn label(self) -> &'static str {
        match self {
            Self::Dsp(op) => op.label(),
            Self::Generator => "Générateur RF",
            Self::Dut => "DUT",
            Self::Analyzer => "Analyseur de spectre",
            Self::Python => "Script Python",
            Self::Peak => "Détection de pic",
            Self::Limit => "Contrôle de limites",
            Self::IqModulator => "Modulateur I/Q",
            Self::Dac => "DAC · N/A",
            Self::Adc => "CAN · A/N",
            Self::Pna => "PNA",
            Self::PnaX => "PNA-X",
            Self::VariableResistor => "Résistance variable",
            Self::Thermometer => "Thermomètre",
            Self::Awg => "AWG",
            Self::NoiseFigureMeter => "Noise Figure Meter",
            Self::PowerMeter => "Power Meter",
            Self::PowerSensor => "Power Sensor",
            Self::Thermostream => "Thermostream",
        }
    }
    pub fn category(self) -> &'static str {
        match self {
            Self::Dsp(op) => op.category(),
            Self::Generator
            | Self::Analyzer
            | Self::Pna
            | Self::PnaX
            | Self::Awg
            | Self::NoiseFigureMeter
            | Self::PowerMeter
            | Self::PowerSensor => "Instruments RF",
            Self::IqModulator | Self::Dac | Self::Adc | Self::VariableResistor => {
                "Électronique & conversion"
            }
            Self::Thermometer | Self::Thermostream => "Thermique",
            Self::Dut => "Composants DUT",
            Self::Python | Self::Peak | Self::Limit => "Analyse & automatisation",
        }
    }
    pub fn tag(self) -> &'static str {
        match self {
            Self::Dsp(_) => "DSP",
            Self::Generator | Self::Awg => "SOURCE",
            Self::Dut => "DUT",
            Self::Analyzer => "SPECTRE",
            Self::Pna | Self::PnaX => "RÉSEAU",
            Self::Python => "PYTHON",
            Self::Peak => "ANALYSE",
            Self::Limit => "TEST",
            Self::IqModulator => "MODULATION",
            Self::Dac | Self::Adc => "CONVERSION",
            Self::VariableResistor => "PASSIF",
            Self::Thermometer | Self::Thermostream => "THERMIQUE",
            Self::NoiseFigureMeter => "BRUIT",
            Self::PowerMeter | Self::PowerSensor => "PUISSANCE",
        }
    }
    pub fn inputs(self) -> &'static [Terminal] {
        use Port::*;
        match self {
            Self::Dsp(op) => op.inputs(),
            Self::Generator | Self::Awg | Self::VariableResistor | Self::Thermostream => &[],
            Self::Dut => &[Terminal {
                name: "RF IN",
                port: Signal,
                required: false,
            }],
            Self::Analyzer | Self::PowerSensor => &[Terminal {
                name: "RF IN",
                port: Signal,
                required: true,
            }],
            Self::Python | Self::Peak => &[Terminal {
                name: "TRACE",
                port: Trace,
                required: true,
            }],
            Self::Limit | Self::PowerMeter => &[Terminal {
                name: "POWER",
                port: Scalar,
                required: true,
            }],
            Self::IqModulator => &[
                Terminal {
                    name: "I",
                    port: Analog,
                    required: true,
                },
                Terminal {
                    name: "Q",
                    port: Analog,
                    required: true,
                },
                Terminal {
                    name: "LO",
                    port: Signal,
                    required: true,
                },
            ],
            Self::Dac => &[Terminal {
                name: "DATA",
                port: Digital,
                required: true,
            }],
            Self::Adc => &[Terminal {
                name: "IN",
                port: Analog,
                required: true,
            }],
            Self::Pna | Self::PnaX | Self::NoiseFigureMeter => &[Terminal {
                name: "DUT",
                port: DutModel,
                required: false,
            }],
            Self::Thermometer => &[Terminal {
                name: "TEMP",
                port: Temperature,
                required: false,
            }],
        }
    }
    pub fn outputs(self) -> &'static [Terminal] {
        use Port::*;
        match self {
            Self::Dsp(op) => op.outputs(),
            Self::Generator | Self::IqModulator => &[Terminal {
                name: "RF OUT",
                port: Signal,
                required: false,
            }],
            Self::Dut => &[
                Terminal {
                    name: "RF OUT",
                    port: Signal,
                    required: false,
                },
                Terminal {
                    name: "MODEL",
                    port: DutModel,
                    required: false,
                },
            ],
            Self::Analyzer | Self::Python => &[Terminal {
                name: "TRACE",
                port: Trace,
                required: false,
            }],
            Self::Peak | Self::Limit | Self::PowerSensor | Self::PowerMeter => &[Terminal {
                name: "POWER",
                port: Scalar,
                required: false,
            }],
            Self::Awg => &[
                Terminal {
                    name: "I",
                    port: Digital,
                    required: false,
                },
                Terminal {
                    name: "Q",
                    port: Digital,
                    required: false,
                },
            ],
            Self::Dac => &[Terminal {
                name: "OUT",
                port: Analog,
                required: false,
            }],
            Self::Adc => &[Terminal {
                name: "DATA",
                port: Digital,
                required: false,
            }],
            Self::Pna | Self::PnaX => &[Terminal {
                name: "S-PARAM",
                port: SParameters,
                required: false,
            }],
            Self::VariableResistor => &[Terminal {
                name: "R",
                port: Resistance,
                required: false,
            }],
            Self::Thermometer | Self::Thermostream => &[Terminal {
                name: "TEMP",
                port: Temperature,
                required: false,
            }],
            Self::NoiseFigureMeter => &[Terminal {
                name: "NF",
                port: NoiseFigure,
                required: false,
            }],
        }
    }
    // First-terminal conveniences retained for existing clients.
    pub fn input(self) -> Option<Port> {
        self.inputs().first().map(|p| p.port)
    }
    pub fn output(self) -> Port {
        self.outputs()[0].port
    }
    pub fn is_extended(self) -> bool {
        !matches!(
            self,
            Self::Generator
                | Self::Dut
                | Self::Analyzer
                | Self::Python
                | Self::Peak
                | Self::Limit
                | Self::Dsp(_)
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Waveform {
    pub samples: Vec<f64>,
    pub sample_rate_hz: f64,
    pub tone_hz: f64,
    pub unit: String,
    pub simulated: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NetworkTrace {
    pub frequency_hz: Vec<f64>,
    pub magnitude_db: Vec<f64>,
    pub phase_deg: Vec<f64>,
    pub parameter: String,
    pub simulated: bool,
}
#[derive(Clone, Debug)]
pub struct Measurement {
    pub name: String,
    pub value: f64,
    pub unit: String,
    pub simulated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub dsp: dsp::Settings,
    pub resource: String,
    pub frequency_hz: f64,
    pub power_dbm: f64,
    pub loss_db: f64,
    pub start_hz: f64,
    pub stop_hz: f64,
    pub points: usize,
    pub lower_dbm: f64,
    pub upper_dbm: f64,
    pub trace_query: String,
    pub script: String,
    pub sample_rate_hz: f64,
    pub tone_hz: f64,
    pub samples: usize,
    pub resolution_bits: u8,
    pub voltage_v: f64,
    pub temperature_c: f64,
    pub resistance_ohm: f64,
    pub noise_figure_db: f64,
    pub s_parameter: String,
    pub dut_id: Option<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self { dsp: dsp::Settings::default(), resource: "SIM::RF::INSTR".into(), frequency_hz: 2.45e9, power_dbm: -10.0,
            loss_db: 3.0, start_hz: 2.40e9, stop_hz: 2.50e9, points: 401,
            lower_dbm: -15.0, upper_dbm: -11.0, trace_query: ":TRAC:DATA? TRACE1".into(),
            sample_rate_hz: 100e6, tone_hz: 1e6, samples: 1024, resolution_bits: 14, voltage_v: 1.0, temperature_c: 25., resistance_ohm: 50., noise_figure_db: 2.5, s_parameter: "S21".into(), dut_id: None,
            script: "output = trace\n# Exemple : compenser une perte de câble de 0.5 dB\n# output['amplitude_dbm'] = [x + 0.5 for x in trace['amplitude_dbm']]\n".into() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Node {
    pub id: u64,
    pub title: String,
    pub kind: Kind,
    pub position: [f32; 2],
    pub config: Config,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub breakpoint: bool,
    #[serde(default)]
    pub probe: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Annotation {
    pub id: u64,
    pub text: String,
    pub position: [f32; 2],
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Edge {
    pub from: u64,
    pub to: u64,
    #[serde(default)]
    pub from_port: usize,
    #[serde(default)]
    pub to_port: usize,
    #[serde(default)]
    pub waypoints: Vec<[f32; 2]>,
    #[serde(default)]
    pub auto_routed: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    #[serde(default)]
    pub annotations: Vec<Annotation>,
}

impl Graph {
    pub fn add(&mut self, kind: Kind, position: [f32; 2]) -> u64 {
        let id = self.nodes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        self.nodes.push(Node {
            id,
            title: kind.label().into(),
            kind,
            position,
            config: Config::default(),
            comment: String::new(),
            breakpoint: false,
            probe: false,
        });
        if let Kind::Dsp(op) = kind {
            let c = &mut self.nodes.last_mut().expect("added node").config.dsp;
            match op {
                dsp::Op::AnalogMod | dsp::Op::AnalogDemod => c.modulation = "AM".into(),
                dsp::Op::Costas => c.order = 4,
                _ => {}
            }
        }
        id
    }
    pub fn remove(&mut self, id: u64) {
        self.nodes.retain(|n| n.id != id);
        self.edges.retain(|e| e.from != id && e.to != id);
    }
    pub fn node(&self, id: u64) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
    pub fn connect(&mut self, from: u64, to: u64) -> Result<()> {
        self.connect_ports(from, 0, to, 0)
    }
    pub fn connect_ports(
        &mut self,
        from: u64,
        from_port: usize,
        to: u64,
        to_port: usize,
    ) -> Result<()> {
        let a = self
            .node(from)
            .ok_or_else(|| Error::Invalid("Bloc source absent".into()))?;
        let b = self
            .node(to)
            .ok_or_else(|| Error::Invalid("Bloc destination absent".into()))?;
        if from == to
            || a.kind.outputs().get(from_port).is_none()
            || b.kind.inputs().get(to_port).is_none()
            || a.kind.outputs()[from_port].port != b.kind.inputs()[to_port].port
        {
            return Err(Error::Invalid("Ports incompatibles".into()));
        }
        if self
            .edges
            .iter()
            .any(|e| e.to == to && e.to_port == to_port)
        {
            return Err(Error::Invalid(
                "Entrée déjà reliée : retirer le câble d'abord".into(),
            ));
        }
        self.edges.push(Edge {
            from,
            to,
            from_port,
            to_port,
            waypoints: Vec::new(),
            auto_routed: false,
        });
        if let Err(e) = self.order() {
            self.edges.pop();
            return Err(e);
        }
        Ok(())
    }
    pub fn order(&self) -> Result<Vec<u64>> {
        let mut annotation_ids = BTreeSet::new();
        if self.annotations.len() > 1000
            || self.annotations.iter().any(|a| {
                a.id == u64::MAX
                    || !annotation_ids.insert(a.id)
                    || a.text.len() > 8192
                    || a.position.iter().any(|p| !p.is_finite() || p.abs() > 1e6)
            })
            || self.nodes.iter().any(|n| n.comment.len() > 8192)
        {
            return Err(Error::Invalid(
                "Annotations invalides ou trop volumineuses".into(),
            ));
        }
        let mut degrees: BTreeMap<_, usize> = self.nodes.iter().map(|n| (n.id, 0)).collect();
        if degrees.len() != self.nodes.len() {
            return Err(Error::Invalid("Identifiants de blocs dupliqués".into()));
        }
        let mut inputs = BTreeSet::new();
        for e in &self.edges {
            if e.waypoints.len() > 64
                || e.waypoints
                    .iter()
                    .flatten()
                    .any(|v| !v.is_finite() || v.abs() > 1e6)
            {
                return Err(Error::Invalid("Points de câble invalides".into()));
            }
            let a = self
                .node(e.from)
                .ok_or_else(|| Error::Invalid("Câble orphelin".into()))?;
            let b = self
                .node(e.to)
                .ok_or_else(|| Error::Invalid("Câble orphelin".into()))?;
            if a.kind.outputs().get(e.from_port).is_none()
                || b.kind.inputs().get(e.to_port).is_none()
                || a.kind.outputs()[e.from_port].port != b.kind.inputs()[e.to_port].port
                || !inputs.insert((e.to, e.to_port))
            {
                return Err(Error::Invalid(
                    "Câble incompatible ou entrée multiple".into(),
                ));
            }
            *degrees.get_mut(&e.to).expect("node exists") += 1;
        }
        let mut queue: VecDeque<_> = degrees
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut ordered = Vec::with_capacity(self.nodes.len());
        while let Some(id) = queue.pop_front() {
            ordered.push(id);
            for edge in self.edges.iter().filter(|e| e.from == id) {
                let d = degrees.get_mut(&edge.to).expect("validated edge");
                *d -= 1;
                if *d == 0 {
                    queue.push_back(edge.to);
                }
            }
        }
        if ordered.len() != self.nodes.len() {
            return Err(Error::Invalid("Cycle détecté dans le schéma".into()));
        }
        Ok(ordered)
    }
    pub fn validate(&self) -> Result<()> {
        self.order()?;
        if self.nodes.is_empty() || self.nodes.len() > 1000 {
            return Err(Error::Invalid(
                "Le banc doit contenir entre 1 et 1000 blocs".into(),
            ));
        }
        for node in &self.nodes {
            let c = &node.config;
            if node.id == u64::MAX
                || node
                    .position
                    .iter()
                    .any(|v| !v.is_finite() || v.abs() > 1e6)
            {
                return Err(Error::Invalid("Position non finie".into()));
            }
            if node.kind.inputs().iter().enumerate().any(|(i, p)| {
                p.required && !self.edges.iter().any(|e| e.to == node.id && e.to_port == i)
            }) {
                return Err(Error::Invalid(format!(
                    "{} : entrée non reliée",
                    node.title
                )));
            }
            let valid = match node.kind {
                Kind::Dsp(_) => c.dsp.validate().is_ok(),
                Kind::Generator => {
                    c.frequency_hz.is_finite()
                        && c.frequency_hz > 0.0
                        && c.power_dbm.is_finite()
                        && (-160.0..=30.0).contains(&c.power_dbm)
                }
                Kind::Dut => {
                    c.loss_db.is_finite()
                        && (0.0..=160.0).contains(&c.loss_db)
                        && c.noise_figure_db.is_finite()
                        && (0. ..=60.).contains(&c.noise_figure_db)
                        && c.frequency_hz.is_finite()
                        && c.frequency_hz > 0.
                        && c.power_dbm.is_finite()
                        && (-160. ..=30.).contains(&c.power_dbm)
                }
                Kind::Analyzer => {
                    c.start_hz.is_finite()
                        && c.stop_hz.is_finite()
                        && c.start_hz > 0.0
                        && c.stop_hz > c.start_hz
                        && (2..=MAX_POINTS).contains(&c.points)
                }
                Kind::Limit => {
                    c.lower_dbm.is_finite() && c.upper_dbm.is_finite() && c.lower_dbm <= c.upper_dbm
                }
                Kind::Python => !c.script.trim().is_empty() && c.script.len() <= 128_000,
                Kind::Peak => true,
                Kind::Pna | Kind::PnaX => {
                    c.loss_db.is_finite()
                        && (0. ..=160.).contains(&c.loss_db)
                        && c.start_hz.is_finite()
                        && c.stop_hz.is_finite()
                        && c.start_hz > 0.
                        && c.stop_hz > c.start_hz
                        && (2..=MAX_POINTS).contains(&c.points)
                        && ["S11", "S21", "S12", "S22"].contains(&c.s_parameter.as_str())
                }
                Kind::Awg | Kind::Dac | Kind::Adc => {
                    c.sample_rate_hz.is_finite()
                        && c.sample_rate_hz > 0.
                        && c.tone_hz.is_finite()
                        && c.tone_hz > 0.
                        && c.tone_hz < c.sample_rate_hz / 2.
                        && (2..=65536).contains(&c.samples)
                        && (2..=24).contains(&c.resolution_bits)
                        && c.voltage_v.is_finite()
                        && c.voltage_v > 0.
                        && c.voltage_v <= 20.
                }
                Kind::IqModulator => c.loss_db.is_finite() && (0. ..=160.).contains(&c.loss_db),
                Kind::VariableResistor => {
                    c.resistance_ohm.is_finite() && (0. ..=1e9).contains(&c.resistance_ohm)
                }
                Kind::Thermometer | Kind::Thermostream => {
                    c.temperature_c.is_finite() && (-100. ..=250.).contains(&c.temperature_c)
                }
                Kind::NoiseFigureMeter => {
                    c.noise_figure_db.is_finite() && (0. ..=60.).contains(&c.noise_figure_db)
                }
                Kind::PowerMeter | Kind::PowerSensor => true,
            };
            if !valid {
                return Err(Error::Invalid(format!(
                    "{} : configuration invalide",
                    node.title
                )));
            }
        }
        Ok(())
    }
    pub fn demo() -> Self {
        let mut g = Self::default();
        for (kind, p) in [
            (Kind::Generator, [36., 80.]),
            (Kind::Dut, [294., 80.]),
            (Kind::Analyzer, [552., 80.]),
            (Kind::Python, [552., 300.]),
            (Kind::Peak, [294., 300.]),
            (Kind::Limit, [36., 300.]),
        ] {
            g.add(kind, p);
        }
        for (a, b) in [(1, 2), (2, 3), (3, 4), (4, 5), (5, 6)] {
            g.connect(a, b).expect("demo graph");
        }
        g
    }
    pub fn network_demo() -> Self {
        let mut g = Self::default();
        let d = g.add(Kind::Dut, [40., 90.]);
        let p = g.add(Kind::PnaX, [360., 90.]);
        g.connect_ports(d, 1, p, 0).expect("DUT model link");
        g
    }
    pub fn iq_demo() -> Self {
        let mut g = Self::default();
        let a = g.add(Kind::Awg, [20., 20.]);
        let i = g.add(Kind::Dac, [310., 20.]);
        let q = g.add(Kind::Dac, [310., 240.]);
        let lo = g.add(Kind::Generator, [310., 460.]);
        let m = g.add(Kind::IqModulator, [600., 200.]);
        let s = g.add(Kind::Analyzer, [900., 200.]);
        for (f, fp, t, tp) in [
            (a, 0, i, 0),
            (a, 1, q, 0),
            (i, 0, m, 0),
            (q, 0, m, 1),
            (lo, 0, m, 2),
            (m, 0, s, 0),
        ] {
            g.connect_ports(f, fp, t, tp).expect("I/Q link");
        }
        g
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub schema_version: u32,
    pub name: String,
    pub graph: Graph,
}
impl Default for Project {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            name: "Caractérisation d'une chaîne RF · 2.45 GHz".into(),
            graph: Graph::demo(),
        }
    }
}
impl Project {
    pub fn from_json(json: &str) -> Result<Self> {
        if json.len() > 4_000_000 {
            return Err(Error::Invalid("Projet trop volumineux".into()));
        }
        let p: Self = serde_json::from_str(json)?;
        if p.schema_version != SCHEMA_VERSION {
            return Err(Error::Invalid(format!(
                "Version de projet {} non prise en charge",
                p.schema_version
            )));
        }
        // Drafts may have unconnected inputs, but their structure must be sound.
        p.graph.order()?;
        if p.graph.nodes.len() > 1000
            || p.graph.nodes.iter().any(|n| {
                n.id == u64::MAX || n.position.iter().any(|v| !v.is_finite() || v.abs() > 1e6)
            })
        {
            return Err(Error::Invalid("Projet invalide".into()));
        }
        Ok(p)
    }
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}
pub fn self_tests() -> Vec<TestResult> {
    let mut results = Vec::new();
    let mut check = |name: &str, passed: bool, detail: &str| {
        results.push(TestResult {
            name: name.into(),
            passed,
            detail: detail.into(),
        })
    };
    let g = Graph::demo();
    check(
        "Ordonnancement du banc RF",
        g.validate().is_ok() && g.order().ok() == Some(vec![1, 2, 3, 4, 5, 6]),
        "6 blocs exécutés selon les dépendances",
    );
    let mut bad = g.clone();
    check(
        "Refus des ports incompatibles",
        bad.connect(1, 5).is_err(),
        "Un signal RF ne peut pas entrer sur un port Trace",
    );
    let mut cycle = Graph::default();
    let a = cycle.add(Kind::Dut, [0., 0.]);
    let b = cycle.add(Kind::Dut, [1., 1.]);
    cycle.connect(a, b).expect("first edge");
    check(
        "Détection de cycle",
        cycle.connect(b, a).is_err() && cycle.edges.len() == 1,
        "Câble refusé, schéma préservé",
    );
    let trace = Trace {
        frequency_hz: vec![1e9, 2e9, 3e9],
        amplitude_dbm: vec![-30., -10., -20.],
        simulated: true,
    };
    check(
        "Pic et unités RF",
        trace.peak().ok() == Some((2e9, -10.)),
        "Pic attendu : 2 GHz / -10 dBm",
    );
    let mut invalid = trace.clone();
    invalid.amplitude_dbm[1] = f64::NAN;
    check(
        "Refus de données NaN",
        invalid.validate().is_err(),
        "Pas de mesure non finie dans un résultat",
    );
    let p = Project::default();
    check(
        "Sauvegarde / ouverture",
        p.to_json()
            .and_then(|s| Project::from_json(&s))
            .is_ok_and(|v| v == p),
        "Aller-retour JSON versionné",
    );
    let mut bad_config = g;
    bad_config.nodes[2].config.points = MAX_POINTS + 1;
    check(
        "Borne de taille des acquisitions",
        bad_config.validate().is_err(),
        "Au plus 100001 points par trace",
    );
    results
}

#[cfg(test)]
mod extension_tests {
    use super::*;
    #[test]
    fn iq_ports_are_distinct_and_typed() {
        let mut g = Graph::iq_demo();
        g.validate().unwrap();
        assert!(g.connect_ports(1, 0, 5, 0).is_err());
        assert!(g.connect_ports(2, 0, 5, 1).is_err());
        assert!(g.connect_ports(1, 9, 5, 0).is_err());
        g.edges.retain(|e| !(e.to == 5 && e.to_port == 1));
        assert!(g.validate().is_err());
    }
    #[test]
    fn legacy_projects_default_new_fields_and_port_indices() {
        let mut json = serde_json::to_value(Project::default()).unwrap();
        for edge in json["graph"]["edges"].as_array_mut().unwrap() {
            edge.as_object_mut().unwrap().remove("from_port");
            edge.as_object_mut().unwrap().remove("to_port");
        }
        for node in json["graph"]["nodes"].as_array_mut().unwrap() {
            node["config"]
                .as_object_mut()
                .unwrap()
                .remove("sample_rate_hz");
        }
        let p = Project::from_json(&json.to_string()).unwrap();
        p.graph.validate().unwrap();
        assert_eq!(p.graph.nodes[0].config.sample_rate_hz, 100e6);
    }
    #[test]
    fn network_analyzer_uses_dut_model_not_rf_power() {
        let mut g = Graph::network_demo();
        g.validate().unwrap();
        g.edges.clear();
        assert!(g.connect(1, 2).is_err());
        g.connect_ports(1, 1, 2, 0).unwrap();
    }
    #[test]
    fn multiport_cycles_rejected() {
        let mut g = Graph::default();
        let d = g.add(Kind::Dac, [0., 0.]);
        let a = g.add(Kind::Adc, [1., 0.]);
        g.connect(d, a).unwrap();
        assert!(g.connect(a, d).is_err());
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_editor_metadata_defaults_and_new_metadata_roundtrips() {
        let mut json = serde_json::to_value(Project::default()).unwrap();
        json["graph"].as_object_mut().unwrap().remove("annotations");
        for n in json["graph"]["nodes"].as_array_mut().unwrap() {
            for field in ["comment", "breakpoint", "probe"] {
                n.as_object_mut().unwrap().remove(field);
            }
        }
        for e in json["graph"]["edges"].as_array_mut().unwrap() {
            e.as_object_mut().unwrap().remove("auto_routed");
        }
        let mut p = Project::from_json(&json.to_string()).unwrap();
        assert!(p.graph.annotations.is_empty());
        assert!(
            p.graph
                .nodes
                .iter()
                .all(|n| !n.probe && !n.breakpoint && n.comment.is_empty())
        );
        p.graph.nodes[0].comment = "Référence de calibration".into();
        p.graph.nodes[0].breakpoint = true;
        p.graph.nodes[0].probe = true;
        p.graph.annotations.push(Annotation {
            id: 1,
            text: "DUT".into(),
            position: [0., 250.],
        });
        p.graph.edges[0].auto_routed = true;
        assert_eq!(Project::from_json(&p.to_json().unwrap()).unwrap(), p);
    }
    #[test]
    fn annotation_ids_and_positions_are_validated() {
        let mut g = Graph::default();
        g.annotations.push(Annotation {
            id: 1,
            text: "Note".into(),
            position: [0., 0.],
        });
        g.order().unwrap();
        g.annotations.push(g.annotations[0].clone());
        assert!(g.order().is_err());
        g.annotations.pop();
        g.annotations[0].id = u64::MAX;
        assert!(g.order().is_err());
        g.annotations[0].id = 1;
        g.annotations[0].position[0] = f32::NAN;
        assert!(g.order().is_err());
    }
    #[test]
    fn integrated_suite_passes() {
        for r in self_tests() {
            assert!(r.passed, "{}", r.name);
        }
    }
    #[test]
    fn removal_cleans_wires() {
        let mut g = Graph::demo();
        g.remove(3);
        assert!(g.edges.iter().all(|e| e.from != 3 && e.to != 3));
        assert!(g.validate().is_err());
    }
    #[test]
    fn future_schema_rejected() {
        let p = Project {
            schema_version: 2,
            ..Project::default()
        };
        assert!(Project::from_json(&p.to_json().unwrap()).is_err());
    }
    #[test]
    fn csv_preserves_provenance() {
        let t = Trace {
            frequency_hz: vec![1., 2.],
            amplitude_dbm: vec![-5., -2.],
            simulated: true,
        };
        let csv = t.csv().unwrap();
        assert!(csv.starts_with("# simulated=true"));
        assert!(csv.contains("2.000000,-2.000000"));
    }
    #[test]
    fn duplicate_ids_and_dangling_edges_rejected() {
        let mut g = Graph::demo();
        g.nodes[0].id = 2;
        assert!(g.order().is_err());
        let mut g = Graph::demo();
        g.edges[0].from = 1234;
        assert!(g.order().is_err());
    }
    #[test]
    fn invalid_sweep_and_missing_input_rejected() {
        let mut g = Graph::demo();
        g.nodes[2].config.stop_hz = g.nodes[2].config.start_hz;
        assert!(g.validate().is_err());
        g = Graph::demo();
        g.edges.clear();
        assert!(g.validate().is_err());
    }
}
