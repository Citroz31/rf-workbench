//! Physical bench cables are undirected electrical connections, not data-flow
//! dependencies. A two-port VNA/DUT loop must not be mistaken for a DAG cycle.
use crate::{Error, Graph, Kind, Node, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PhysicalPortKind {
    Rf,
    DcPositive,
    DcReturn,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PhysicalDirection {
    Bidirectional,
    Input,
    Output,
}
#[derive(Clone, Debug, PartialEq)]
pub struct PhysicalPort {
    pub name: String,
    pub kind: PhysicalPortKind,
    pub direction: PhysicalDirection,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PhysicalConnection {
    pub from: u64,
    pub from_port: usize,
    pub to: u64,
    pub to_port: usize,
    #[serde(default)]
    pub waypoints: Vec<[f32; 2]>,
    #[serde(default)]
    pub auto_routed: bool,
}
pub fn s_parameter_ports(value: &str) -> Option<(u8, u8)> {
    let p = value.as_bytes();
    (p.len() == 3 && p[0] == b'S' && (b'1'..=b'4').contains(&p[1]) && (b'1'..=b'4').contains(&p[2]))
        .then(|| (p[1] - b'0', p[2] - b'0'))
}
impl Node {
    pub fn dut_rf_path(&self) -> (usize, usize) {
        if self.config.dut_id.as_deref() == Some("macom-cgy2170yhv-c1") {
            if self.config.dut_mode == "TX" {
                (2, 1)
            } else {
                (0, 2)
            }
        } else {
            (0, 1)
        }
    }
    pub fn physical_ports(&self) -> Vec<PhysicalPort> {
        use PhysicalDirection::*;
        use PhysicalPortKind::*;
        let p = |name: &str, kind, direction| PhysicalPort {
            name: name.into(),
            kind,
            direction,
        };
        if let Some(max) = self.kind.max_rf_ports() {
            return (1..=self.config.instrument.port_count.min(max))
                .map(|i| p(&format!("PORT {i}"), Rf, Bidirectional))
                .collect();
        }
        match self.kind {
            Kind::Dut if self.config.dut_id.as_deref() == Some("macom-cgy2170yhv-c1") => vec![
                p("RX IN", Rf, Input),
                p("TX OUT", Rf, Output),
                p("COM RF", Rf, Bidirectional),
                p("VD +", DcPositive, Input),
                p("VS −", DcPositive, Input),
                p("RETURN", DcReturn, Input),
            ],
            Kind::Dut if self.config.dut_id.as_deref() == Some("macom-maal-fr1245") => vec![
                p("RF IN", Rf, Input),
                p("RF OUT", Rf, Output),
                p("VD +", DcPositive, Input),
                p("VG −", DcPositive, Input),
                p("RETURN", DcReturn, Input),
            ],
            Kind::Dut => vec![
                p("RF IN", Rf, Input),
                p("RF OUT", Rf, Output),
                p("DC +", DcPositive, Input),
                p("DC RETURN", DcReturn, Input),
            ],
            Kind::DcSupplyE3631A => vec![
                p("P6V", DcPositive, Output),
                p("6V −", DcReturn, Output),
                p("P25V", DcPositive, Output),
                p("COM +25", DcReturn, Output),
                p("N25V", DcPositive, Output),
                p("COM −25", DcReturn, Output),
            ],
            Kind::DcSupplyE36313A => (1..=3)
                .flat_map(|i| {
                    [
                        p(&format!("CH{i} +"), DcPositive, Output),
                        p(&format!("CH{i} −"), DcReturn, Output),
                    ]
                })
                .collect(),
            _ => Vec::new(),
        }
    }
}
impl Graph {
    pub fn validate_physical(&self) -> Result<()> {
        if self.physical_connections.len() > 6000 {
            return Err(Error::Invalid("Trop de câbles physiques".into()));
        }
        let mut used = BTreeSet::new();
        for cable in &self.physical_connections {
            let a = self
                .node(cable.from)
                .ok_or_else(|| Error::Invalid("Câble physique orphelin".into()))?;
            let b = self
                .node(cable.to)
                .ok_or_else(|| Error::Invalid("Câble physique orphelin".into()))?;
            let pa = a.physical_ports();
            let pb = b.physical_ports();
            let (Some(pa), Some(pb)) = (pa.get(cable.from_port), pb.get(cable.to_port)) else {
                return Err(Error::Invalid("Broche physique absente".into()));
            };
            if cable.from == cable.to
                || pa.kind != pb.kind
                || (pa.kind != PhysicalPortKind::Rf && pa.direction == pb.direction)
                || !used.insert((cable.from, cable.from_port))
                || !used.insert((cable.to, cable.to_port))
                || cable.waypoints.len() > 64
                || cable
                    .waypoints
                    .iter()
                    .flatten()
                    .any(|v| !v.is_finite() || v.abs() > 1e6)
            {
                return Err(Error::Invalid(
                    "Câble physique incompatible ou broche déjà connectée".into(),
                ));
            }
        }
        Ok(())
    }
    pub fn connect_physical(
        &mut self,
        from: u64,
        from_port: usize,
        to: u64,
        to_port: usize,
    ) -> Result<()> {
        self.physical_connections.push(PhysicalConnection {
            from,
            from_port,
            to,
            to_port,
            waypoints: Vec::new(),
            auto_routed: false,
        });
        if let Err(e) = self.validate_physical() {
            self.physical_connections.pop();
            return Err(e);
        }
        Ok(())
    }
    pub fn remove_physical(&mut self, index: usize) -> Option<PhysicalConnection> {
        (index < self.physical_connections.len()).then(|| self.physical_connections.remove(index))
    }
    /// Return the count of cables removed because their VNA port disappeared.
    pub fn set_vna_ports(&mut self, id: u64, count: u8) -> Result<usize> {
        let n = self
            .nodes
            .iter_mut()
            .find(|n| n.id == id)
            .ok_or_else(|| Error::Invalid("VNA absent".into()))?;
        let max = n
            .kind
            .max_rf_ports()
            .ok_or_else(|| Error::Invalid("Ce bloc n'est pas un VNA".into()))?;
        if !(1..=max).contains(&count) {
            return Err(Error::Invalid(format!(
                "Nombre de ports requis : 1 à {max}"
            )));
        }
        n.config.instrument.port_count = count;
        if !s_parameter_ports(&n.config.s_parameter).is_some_and(|(i, j)| i <= count && j <= count)
        {
            n.config.s_parameter = "S11".into();
        }
        let previous = self.physical_connections.len();
        self.physical_connections.retain(|c| {
            !(c.from == id && c.from_port >= count as usize
                || c.to == id && c.to_port >= count as usize)
        });
        Ok(previous - self.physical_connections.len())
    }
    /// Resolve a forward DUT path between the source j and receiver i of Sij.
    /// Only a single directly wired DUT is modelled; arbitrary circuits require
    /// a solver, so ambiguous or incomplete physical wiring is rejected.
    pub fn vna_dut(&self, vna: u64, parameter: &str) -> Result<Option<&Node>> {
        let (receiver, source) = s_parameter_ports(parameter)
            .ok_or_else(|| Error::Invalid("Paramètre S invalide".into()))?;
        let peer = |port: usize| {
            self.physical_connections.iter().find_map(|c| {
                if c.from == vna && c.from_port == port {
                    Some((c.to, c.to_port))
                } else if c.to == vna && c.to_port == port {
                    Some((c.from, c.from_port))
                } else {
                    None
                }
            })
        };
        let source_peer = peer((source - 1) as usize);
        let receiver_peer = peer((receiver - 1) as usize);
        if source_peer.is_none() && receiver_peer.is_none() {
            return Ok(None);
        }
        let Some((dut, pin)) = source_peer else {
            return Err(Error::Invalid("Port source VNA non câblé".into()));
        };
        let dut = self
            .node(dut)
            .filter(|n| n.kind == Kind::Dut)
            .ok_or_else(|| {
                Error::Invalid("Simulation VNA : connecter directement un DUT".into())
            })?;
        let (input, output) = dut.dut_rf_path();
        if receiver == source && (pin == input || pin == output) {
            return Ok(Some(dut));
        }
        if !(pin == input && receiver_peer == Some((dut.id, output))
            || pin == output && receiver_peer == Some((dut.id, input)))
        {
            return Err(Error::Invalid("Simulation VNA : relier source → entrée et sortie → récepteur du DUT (mode RX/TX sélectionné)".into()));
        }
        Ok(Some(dut))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vna_physical_loop_does_not_create_data_cycle() {
        let mut g = Graph::default();
        let v = g.add(Kind::PnaX, [0., 0.]);
        let d = g.add(Kind::Dut, [300., 0.]);
        g.connect_physical(v, 0, d, 0).unwrap();
        g.connect_physical(d, 1, v, 1).unwrap();
        g.validate().unwrap();
        assert_eq!(g.vna_dut(v, "S21").unwrap().unwrap().id, d);
        assert!(g.connect_physical(v, 1, d, 0).is_err());
        assert!(g.connect_ports(v, 0, d, 0).is_err());
        assert_eq!(g.set_vna_ports(v, 1).unwrap(), 1);
        assert_eq!(g.node(v).unwrap().config.s_parameter, "S11");
    }
    #[test]
    fn four_port_trace_and_usb_limits() {
        let mut g = Graph::default();
        let v = g.add(Kind::PnaX, [0., 0.]);
        g.set_vna_ports(v, 4).unwrap();
        g.nodes[0].config.s_parameter = "S43".into();
        g.validate().unwrap();
        let u = g.add(Kind::UsbVna, [300., 0.]);
        assert!(g.set_vna_ports(u, 3).is_err());
        g.nodes[1].config.s_parameter = "S33".into();
        assert!(g.validate().is_err());
    }
    #[test]
    fn physical_cables_round_trip_and_legacy_default() {
        let mut g = Graph::default();
        let v = g.add(Kind::Pna, [0., 0.]);
        let d = g.add(Kind::Dut, [300., 0.]);
        g.connect_physical(v, 0, d, 0).unwrap();
        let json = serde_json::to_string(&g).unwrap();
        assert_eq!(g, serde_json::from_str(&json).unwrap());
        let mut old: serde_json::Value = serde_json::from_str(&json).unwrap();
        old.as_object_mut().unwrap().remove("physical_connections");
        assert!(
            serde_json::from_value::<Graph>(old)
                .unwrap()
                .physical_connections
                .is_empty()
        );
        g.remove(d);
        assert!(g.physical_connections.is_empty());
    }
}
