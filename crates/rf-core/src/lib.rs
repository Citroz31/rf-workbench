//! Platform-independent RF data, typed acyclic graphs and versioned bench files.
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
}
impl Port {
    pub fn label(self) -> &'static str {
        match self {
            Self::Signal => "RF",
            Self::Trace => "Trace",
            Self::Scalar => "dBm",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Kind {
    Generator,
    Dut,
    Analyzer,
    Python,
    Peak,
    Limit,
}
impl Kind {
    pub const ALL: [Self; 6] = [
        Self::Generator,
        Self::Dut,
        Self::Analyzer,
        Self::Python,
        Self::Peak,
        Self::Limit,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Generator => "Générateur RF",
            Self::Dut => "Dispositif sous test",
            Self::Analyzer => "Analyseur de spectre",
            Self::Python => "Script Python",
            Self::Peak => "Détection de pic",
            Self::Limit => "Contrôle de limites",
        }
    }
    pub fn tag(self) -> &'static str {
        match self {
            Self::Generator => "SOURCE",
            Self::Dut => "DUT",
            Self::Analyzer => "ACQUISITION",
            Self::Python => "PYTHON",
            Self::Peak => "ANALYSE",
            Self::Limit => "TEST",
        }
    }
    pub fn input(self) -> Option<Port> {
        match self {
            Self::Generator => None,
            Self::Dut | Self::Analyzer => Some(Port::Signal),
            Self::Python | Self::Peak => Some(Port::Trace),
            Self::Limit => Some(Port::Scalar),
        }
    }
    pub fn output(self) -> Port {
        match self {
            Self::Generator | Self::Dut => Port::Signal,
            Self::Analyzer | Self::Python => Port::Trace,
            Self::Peak | Self::Limit => Port::Scalar,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Config {
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
}
impl Default for Config {
    fn default() -> Self {
        Self { resource: "SIM::RF::INSTR".into(), frequency_hz: 2.45e9, power_dbm: -10.0,
            loss_db: 3.0, start_hz: 2.40e9, stop_hz: 2.50e9, points: 401,
            lower_dbm: -15.0, upper_dbm: -11.0, trace_query: ":TRAC:DATA? TRACE1".into(),
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
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Edge {
    pub from: u64,
    pub to: u64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
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
        });
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
        let a = self
            .node(from)
            .ok_or_else(|| Error::Invalid("Bloc source absent".into()))?;
        let b = self
            .node(to)
            .ok_or_else(|| Error::Invalid("Bloc destination absent".into()))?;
        if from == to || b.kind.input() != Some(a.kind.output()) {
            return Err(Error::Invalid("Ports incompatibles".into()));
        }
        if self.edges.iter().any(|e| e.to == to) {
            return Err(Error::Invalid(
                "Entrée déjà reliée : retirer le câble d'abord".into(),
            ));
        }
        self.edges.push(Edge { from, to });
        if let Err(e) = self.order() {
            self.edges.pop();
            return Err(e);
        }
        Ok(())
    }
    pub fn order(&self) -> Result<Vec<u64>> {
        let mut degrees: BTreeMap<_, usize> = self.nodes.iter().map(|n| (n.id, 0)).collect();
        if degrees.len() != self.nodes.len() {
            return Err(Error::Invalid("Identifiants de blocs dupliqués".into()));
        }
        let mut inputs = BTreeSet::new();
        for e in &self.edges {
            let a = self
                .node(e.from)
                .ok_or_else(|| Error::Invalid("Câble orphelin".into()))?;
            let b = self
                .node(e.to)
                .ok_or_else(|| Error::Invalid("Câble orphelin".into()))?;
            if b.kind.input() != Some(a.kind.output()) || !inputs.insert(e.to) {
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
            if node.kind.input().is_some() && !self.edges.iter().any(|e| e.to == node.id) {
                return Err(Error::Invalid(format!(
                    "{} : entrée non reliée",
                    node.title
                )));
            }
            let valid = match node.kind {
                Kind::Generator => {
                    c.frequency_hz.is_finite()
                        && c.frequency_hz > 0.0
                        && c.power_dbm.is_finite()
                        && (-160.0..=30.0).contains(&c.power_dbm)
                }
                Kind::Dut => c.loss_db.is_finite() && (0.0..=160.0).contains(&c.loss_db),
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
mod tests {
    use super::*;
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
