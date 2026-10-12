//! One channel plan shared by the display, simulator and SCPI driver.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arrangement {
    Single,
    Two,
    #[default]
    Quad,
}
impl Arrangement {
    pub fn grid(self) -> (usize, usize) {
        match self { Self::Single => (1, 1), Self::Two => (2, 1), Self::Quad => (2, 2) }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format { #[default] Magnitude, Phase, Smith, Raw }
impl Format {
    pub fn scpi(self) -> &'static str {
        match self { Self::Magnitude | Self::Raw => "MLOG", Self::Phase => "PHAS", Self::Smith => "SMIT" }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayTrace {
    pub parameter: String,
    pub format: Format,
    pub automatic: bool,
    pub per_division: f64,
    pub reference: f64,
    /// Horizontal divisions counted from the bottom, as on the PNA.
    pub reference_position: f64,
}
impl Default for DisplayTrace {
    fn default() -> Self { Self { parameter: "S21".into(), format: Format::Magnitude,
        automatic: true, per_division: 10., reference: 0., reference_position: 5. } }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DisplayWindow { pub traces: Vec<DisplayTrace>, pub active_trace: usize }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Plan {
    pub parameters: Vec<String>,
    pub arrangement: Arrangement,
    pub windows: Vec<DisplayWindow>,
    /// Only applied by an explicit Configure operation or configure_sweep.
    pub synchronize_display: bool,
    /// Existing instrument window numbers reserved for this block, or new ones.
    pub first_window: u32,
}
impl Default for Plan {
    fn default() -> Self { Self::s_parameters(2) }
}
impl Plan {
    pub fn s_parameters(ports: u8) -> Self {
        let parameters = ["S11", "S12", "S21", "S22"].into_iter()
            .filter(|p| valid_parameter(p, ports)).map(str::to_owned).collect::<Vec<_>>();
        let arrangement=if parameters.len()==1 {Arrangement::Single}else{Arrangement::Quad};
        let mut plan = Self { parameters, arrangement,
            windows: Vec::new(), synchronize_display: true, first_window: 1 };
        plan.rebuild_windows(); plan
    }
    pub fn power(source: u8, receiver: u8) -> Self {
        let receiver_name = ["A", "B", "C", "D"][usize::from(receiver - 1)];
        let mut plan = Self { parameters: vec![format!("S{receiver}{source}"),
            format!("S{source}{source}"), format!("R{source}_{source}"), format!("{receiver_name}_{source}")],
            ..Self::s_parameters(2) };
        plan.rebuild_windows(); plan
    }
    /// Checkbox edits preserve existing trace formats/scales. Unselected traces
    /// are removed; explicitly arranged overlays are retained until rebuilt.
    pub fn rebuild_windows(&mut self) {
        let old: Vec<_> = self.windows.iter().flat_map(|w| w.traces.clone()).collect();
        self.windows = self.parameters.iter().map(|p| DisplayWindow {
            traces: vec![old.iter().find(|t| &t.parameter == p).cloned().unwrap_or_else(|| DisplayTrace {
                parameter: p.clone(), format: if p.starts_with('S') { Format::Magnitude } else { Format::Raw },
                ..DisplayTrace::default() })], active_trace: 0 }).collect();
    }
    pub fn validate(&self, ports: u8, power_source: Option<u8>) -> Result<(), String> {
        let mut seen = std::collections::BTreeSet::new();
        if self.parameters.is_empty() || self.parameters.len() > 32
            || !self.parameters.iter().any(|p|p.starts_with('S'))
            || !(1..=1024).contains(&self.first_window)
            || self.windows.is_empty() || self.windows.len() > 16
            || self.parameters.iter().any(|p| !valid_parameter(p, ports) || !seen.insert(p))
            || self.windows.iter().any(|w| w.traces.is_empty() || w.traces.len() > 16
                || w.active_trace >= w.traces.len() || w.traces.iter().any(|t|
                    !self.parameters.contains(&t.parameter) || !t.per_division.is_finite()
                    || !(1e-9..=1e12).contains(&t.per_division) || !t.reference.is_finite()
                    || t.reference.abs() > 1e15 || !t.reference_position.is_finite()
                    || !(0. ..=10.).contains(&t.reference_position)
                    || t.format==Format::Smith && !crate::physical::s_parameter_ports(&t.parameter).is_some_and(|(r,s)|r==s)
                    || !t.parameter.starts_with('S') && matches!(t.format, Format::Phase | Format::Smith)))
        { return Err("Plan de traces/fenêtres PNA invalide".into()); }
        if let Some(source) = power_source
            && self.parameters.iter().any(|p| source_port(p) != Some(source))
        { return Err("Sweep puissance : chaque mesure doit utiliser le port source sélectionné".into()); }
        Ok(())
    }
}
pub fn source_port(parameter: &str) -> Option<u8> {
    crate::physical::s_parameter_ports(parameter).map(|(_, s)| s).or_else(||
        parameter.rsplit_once('_').and_then(|(_, s)| s.parse().ok()))
}
pub fn receiver_port(parameter: &str) -> Option<u8> {
    let (receiver, _) = parameter.split_once('_')?;
    match receiver { "A" => Some(1), "B" => Some(2), "C" => Some(3), "D" => Some(4),
        _ => receiver.strip_prefix('R')?.parse().ok() }
}
pub fn valid_parameter(p: &str, ports: u8) -> bool {
    if let Some((r, s)) = crate::physical::s_parameter_ports(p) { return r <= ports && s <= ports; }
    source_port(p).is_some_and(|s| (1..=ports).contains(&s))
        && receiver_port(p).is_some_and(|r| (1..=ports).contains(&r))
}
pub fn selected(c: &crate::Config) -> Vec<String> {
    if c.instrument.pna.all_s_parameters && !c.instrument.pna.power_sweep {
        (1..=c.instrument.port_count).flat_map(|i|
            (1..=c.instrument.port_count).map(move |j| format!("S{i}{j}"))).collect()
    } else if let Some(plan) = &c.instrument.pna.plan { plan.parameters.clone() }
    else { vec![c.s_parameter.clone()] }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PathSettings {
    pub apply: bool,
    pub configuration: String,
    pub elements: std::collections::BTreeMap<String, String>,
}
impl PathSettings {
    pub fn validate(&self) -> bool {
        self.elements.len() <= 128 && crate::instrument::scpi_name(&self.configuration)
            && self.elements.iter().all(|(k,v)| !k.is_empty() && !v.is_empty()
                && crate::instrument::scpi_name(k) && crate::instrument::scpi_name(v))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn defaults_and_power_plans_have_a_single_channel_and_valid_excitation() {
        let p=Plan::default(); assert_eq!(p.parameters,["S11","S12","S21","S22"]);
        assert!(p.windows.iter().all(|w|w.traces.len()==1)); p.validate(2,None).unwrap();
        let p=Plan::power(1,2); p.validate(2,Some(1)).unwrap(); assert!(p.validate(2,Some(2)).is_err());
        Plan::s_parameters(1).validate(1,None).unwrap();
    }
}
