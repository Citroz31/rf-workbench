//! A preflight estimate, not a circuit solver or an instrument safety rating.
use crate::{Graph, Kind, Node};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Limits {
    pub band_hz: Option<[f64; 2]>,
    pub max_input_dbm: Option<f64>,
    pub max_output_dbm: Option<f64>,
    pub output_p1db_dbm: Option<f64>,
    pub source: String,
}
impl Limits {
    pub fn validate(&self) -> Result<(), String> {
        if self
            .band_hz
            .is_some_and(|b| b.iter().any(|v| !v.is_finite() || *v < 0.) || b[1] <= b[0])
            || [
                self.max_input_dbm,
                self.max_output_dbm,
                self.output_p1db_dbm,
            ]
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || !(-200. ..=100.).contains(v))
            || self.source.len() > 1024
        {
            return Err("Limites RF invalides".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}
#[derive(Clone, Debug)]
pub struct Finding {
    pub severity: Severity,
    pub node: Option<u64>,
    pub message: String,
}
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub findings: Vec<Finding>,
}
impl Report {
    pub fn blocked(&self) -> bool {
        self.findings.iter().any(|f| f.severity == Severity::Error)
    }
    fn add(&mut self, severity: Severity, node: Option<u64>, message: String) {
        if !self
            .findings
            .iter()
            .any(|f| f.node == node && f.message == message)
        {
            self.findings.push(Finding {
                severity,
                node,
                message,
            });
        }
    }
    pub fn inspect(g: &Graph, profiles: &BTreeMap<u64, Limits>) -> Self {
        let mut r = Self::default();
        if let Err(e) = g.validate() {
            r.add(Severity::Error, None, e.to_string());
        }
        let limits = |n: &Node| {
            profiles
                .get(&n.id)
                .cloned()
                .unwrap_or_else(|| n.config.limits.clone())
        };
        for n in &g.nodes {
            let l = limits(n);
            if let Err(e) = l.validate() {
                r.add(Severity::Error, Some(n.id), e);
            }
            if n.kind.max_rf_ports().is_some() {
                let c = &n.config;
                let model = g
                    .edges
                    .iter()
                    .find(|e| e.to == n.id && e.to_port == 0)
                    .and_then(|e| g.node(e.from));
                let connected = g.physical_connections.iter().filter_map(|e| {
                    if e.from == n.id {
                        g.node(e.to)
                    } else if e.to == n.id {
                        g.node(e.from)
                    } else {
                        None
                    }
                });
                for d in connected.chain(model) {
                    if let Some(b) = limits(d).band_hz {
                        let band = if c.instrument.pna.power_sweep {
                            [c.frequency_hz, c.frequency_hz]
                        } else {
                            [c.start_hz, c.stop_hz]
                        };
                        if band[0] < b[0] || band[1] > b[1] {
                            r.add(
                                Severity::Error,
                                Some(d.id),
                                format!(
                                    "{} : balayage {:.6}–{:.6} GHz hors bande {:.6}–{:.6} GHz",
                                    d.title,
                                    band[0] / 1e9,
                                    band[1] / 1e9,
                                    b[0] / 1e9,
                                    b[1] / 1e9
                                ),
                            );
                        }
                    }
                }
                if c.instrument.pna.power_sweep && c.instrument.pna.fixture.enabled {
                    r.add(
                        Severity::Error,
                        Some(n.id),
                        "Deembedding fréquentiel incompatible avec un sweep de puissance".into(),
                    );
                }
            }
        }
        // Propagate the worst source level through modeled RF paths only. Physical
        // cable endpoints carry port indices; reversed active DUTs are not modeled.
        for src in g
            .nodes
            .iter()
            .filter(|n| n.kind == Kind::Generator || n.kind.max_rf_ports().is_some())
        {
            let c = &src.config;
            let (band, power) = if src.kind == Kind::Generator {
                ([c.frequency_hz, c.frequency_hz], c.power_dbm)
            } else if c.instrument.pna.power_sweep {
                (
                    [c.frequency_hz, c.frequency_hz],
                    c.instrument.pna.power_stop_dbm,
                )
            } else {
                ([c.start_hz, c.stop_hz], c.power_dbm)
            };
            let mut queue = vec![(src.id, power, BTreeSet::new())];
            while let Some((id, p, mut seen)) = queue.pop() {
                if !seen.insert(id) {
                    continue;
                }
                let Some(node) = g.node(id) else { continue };
                let mut targets: Vec<(u64, Option<usize>)> = g
                    .edges
                    .iter()
                    .filter(|e| {
                        e.from == id
                            && e.from_port == 0
                            && (e.to_port == 0
                                || g.node(e.to).is_some_and(|n| n.kind == Kind::IqModulator)
                                    && e.to_port == 2)
                    })
                    .map(|e| (e.to, None))
                    .collect();
                for e in &g.physical_connections {
                    let endpoint = if e.from == id {
                        Some((e.from_port, e.to, e.to_port))
                    } else if e.to == id {
                        Some((e.to_port, e.from, e.from_port))
                    } else {
                        None
                    };
                    if let Some((port, to, to_port)) = endpoint
                        && (id == src.id || node.kind == Kind::Dut && port == node.dut_rf_path().1)
                        && node
                            .physical_ports()
                            .get(port)
                            .is_some_and(|p| p.kind == crate::physical::PhysicalPortKind::Rf)
                    {
                        targets.push((to, Some(to_port)));
                    }
                }
                for (to, port) in targets {
                    if seen.contains(&to) {
                        continue;
                    }
                    let Some(d) = g.node(to) else { continue };
                    let l = limits(d);
                    if let Some(b) = l.band_hz {
                        if band[0] < b[0] || band[1] > b[1] {
                            r.add(
                                Severity::Error,
                                Some(to),
                                format!("{} : fréquence source hors bande RF déclarée", d.title),
                            );
                        }
                    } else if d.kind == Kind::Dut {
                        r.add(
                            Severity::Warning,
                            Some(to),
                            format!("{} : bande d'utilisation inconnue", d.title),
                        );
                    }
                    if let Some(max) = l.max_input_dbm {
                        if p > max {
                            r.add(
                                Severity::Error,
                                Some(to),
                                format!(
                                    "{} : entrée estimée {p:.2} dBm > limite {max:.2} dBm",
                                    d.title
                                ),
                            );
                        }
                    } else {
                        r.add(
                            Severity::Warning,
                            Some(to),
                            format!(
                                "{} : puissance d'entrée maximale inconnue (estimation {p:.2} dBm)",
                                d.title
                            ),
                        );
                    }
                    if d.kind == Kind::Dut {
                        if port.is_some_and(|port| port != d.dut_rf_path().0) {
                            r.add(Severity::Warning,Some(to),format!("{} : excitation inverse ; gain et overload de ce trajet non évalués",d.title));
                            continue;
                        }
                        let output = p - d.config.loss_db - d.config.attenuation_db;
                        if let Some(max) = l.max_output_dbm
                            && output > max
                        {
                            r.add(Severity::Error,Some(to),format!("{} : sortie linéaire estimée {output:.2} dBm > limite {max:.2} dBm",d.title));
                        }
                        if let Some(p1) = l.output_p1db_dbm
                            && output > p1
                        {
                            r.add(Severity::Warning,Some(to),format!("{} : compression probable (sortie linéaire {output:.2} dBm, OP1dB {p1:.2} dBm). P1dB n'est pas une limite de dommage.",d.title));
                        }
                        queue.push((to, output, seen.clone()));
                    }
                }
            }
        }
        if r.findings.is_empty() {
            r.add(Severity::Info,None,"Structure et limites renseignées compatibles ; les limites matérielles non renseignées restent à vérifier.".into());
        }
        r
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incompatible_band_and_overload_block_but_p1db_does_not() {
        let mut g = Graph::demo();
        let d = g.nodes.iter_mut().find(|n| n.kind == Kind::Dut).unwrap();
        d.config.limits = Limits {
            band_hz: Some([8e9, 12e9]),
            max_input_dbm: Some(-20.),
            ..Default::default()
        };
        let r = Report::inspect(&g, &BTreeMap::new());
        assert!(r.blocked());
        assert!(r.findings.iter().any(|f| f.message.contains("limite")));
        g.nodes[1].config.limits = Limits {
            output_p1db_dbm: Some(-30.),
            ..Default::default()
        };
        assert!(!Report::inspect(&g, &BTreeMap::new()).blocked());
    }
}
