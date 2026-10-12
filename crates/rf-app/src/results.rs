use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format {
    Magnitude,
    Phase,
    Smith,
    Linear,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Window {
    pub id: u64,
    pub node: u64,
    pub parameter: String,
    pub format: Format,
    pub rect: [f32; 4],
    pub corrected: bool,
    #[serde(default)]
    pub traces: Vec<Trace>,
    #[serde(default)]
    pub active_trace: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Scale {
    pub automatic: bool,
    pub per_division: f64,
    pub reference: f64,
    pub reference_division: f64,
}
impl Default for Scale {
    fn default() -> Self {
        Self { automatic: true, per_division: 10., reference: 0., reference_division: 5. }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds { pub low: f64, pub high: f64 }
impl Bounds {
    pub fn normalized(self, value: f64) -> f64 { (value-self.low)/(self.high-self.low) }
}
impl Scale {
    pub fn bounds(&self, values: Option<&[f64]>) -> Bounds {
        if self.automatic && let Some(y)=values.filter(|y|!y.is_empty()&&y.iter().all(|v|v.is_finite())) {
            let min=y.iter().copied().fold(f64::INFINITY,f64::min);
            let max=y.iter().copied().fold(f64::NEG_INFINITY,f64::max);
            let pad=((max-min)*0.1).max(0.1);
            return Bounds { low:min-pad, high:max+pad };
        }
        Bounds { low:self.reference-(10.-self.reference_division)*self.per_division,
            high:self.reference+self.reference_division*self.per_division }
    }
    fn valid(&self) -> bool {
        self.per_division.is_finite() && (1e-9..=1e12).contains(&self.per_division)
            && self.reference.is_finite() && self.reference.abs()<=1e15
            && self.reference_division.is_finite() && (0. ..=10.).contains(&self.reference_division)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trace {
    pub node: u64,
    pub parameter: String,
    pub format: Format,
    pub corrected: bool,
    pub visible: bool,
    pub scale: Scale,
    pub color: usize,
}
impl Window {
    pub fn new(id:u64,node:u64,parameter:String,format:Format,corrected:bool) -> Self {
        Self { id,node,parameter,format,corrected,rect:[250.,210.,540.,290.],traces:Vec::new(),active_trace:0 }
    }
    pub fn ensure_traces(&mut self) {
        if self.traces.is_empty() {
            self.traces.push(Trace {node:self.node,parameter:self.parameter.clone(),format:self.format,
                corrected:self.corrected,visible:true,scale:Scale::default(),color:0});
        }
        self.active_trace=self.active_trace.min(self.traces.len()-1);
    }
    pub fn add_trace(&mut self,node:u64,parameter:String,format:Format,corrected:bool) {
        self.ensure_traces();
        if let Some(i)=self.traces.iter().position(|t|t.node==node&&t.parameter==parameter&&t.format==format&&t.corrected==corrected) {
            self.active_trace=i;return;
        }
        if self.traces.len()>=16{return;}
        self.active_trace=self.traces.len();
        self.traces.push(Trace{node,parameter,format,corrected,visible:true,scale:Scale::default(),color:self.active_trace});
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Panel {
    pub open: bool,
    pub floating: bool,
    pub columns: usize,
    pub rows: usize,
    pub page: usize,
    pub selected: u64,
    pub rect: [f32;4],
}
impl Default for Panel {
    fn default() -> Self {
        Self { open:true, floating:false, columns:2, rows:2, page:0, selected:0, rect:[230.,170.,1140.,700.] }
    }
}
impl Panel {
    pub fn validate(&self) -> Result<(),String> {
        if !(1..=4).contains(&self.columns) || !(1..=4).contains(&self.rows) || self.page>31
            || self.rect.iter().any(|v|!v.is_finite()||v.abs()>20000.) || self.rect[2]<400. || self.rect[3]<300. {
            return Err("Disposition du panneau VNA invalide".into());
        }
        Ok(())
    }
}
pub fn validate(w: &[Window]) -> Result<(), String> {
    let mut ids = std::collections::BTreeSet::new();
    if w.len() > 32
        || w.iter().any(|p| {
            !ids.insert(p.id)
                || p.parameter.len() > 256
                || p.rect.iter().any(|v| !v.is_finite() || v.abs() > 20000.)
                || p.rect[2] < 200.
                || p.rect[3] < 160.
                || p.traces.len()>16
                || (!p.traces.is_empty() && p.active_trace>=p.traces.len())
                || p.traces.iter().any(|t|t.parameter.is_empty()||t.parameter.len()>256||!t.scale.valid()||t.color>255)
        })
    {
        return Err("Fenêtres de résultats invalides".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn switching_the_active_trace_keeps_other_trace_positions_and_scales() {
        let mut w=Window::new(1,2,"S11".into(),Format::Magnitude,false);
        w.add_trace(2,"S22".into(),Format::Magnitude,false);
        w.traces[0].scale=Scale{automatic:false,per_division:5.,reference:-20.,reference_division:5.};
        w.traces[1].scale=Scale{automatic:false,per_division:1.,reference:-5.,reference_division:5.};
        let before=w.traces[1].scale.bounds(None).normalized(-3.);
        w.active_trace=0;w.traces[0].scale.per_division=10.;
        assert_eq!(w.traces[1].scale.bounds(None).normalized(-3.),before);
        assert_eq!(w.traces[1].scale.per_division,1.);
        w.active_trace=1;
        let json=serde_json::to_string(&w).unwrap();
        let loaded:Window=serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.active_trace,1);
        assert_eq!(loaded.traces[0].scale.per_division,10.);
        assert_eq!(loaded.traces[1].scale.bounds(None).normalized(-3.),before);
    }
    #[test]
    fn legacy_single_trace_windows_and_invalid_scales_are_handled() {
        let mut w:Window=serde_json::from_str(r#"{"id":1,"node":2,"parameter":"S11","format":"Magnitude","rect":[0,0,540,290],"corrected":false}"#).unwrap();
        w.ensure_traces();assert_eq!(w.traces[0].parameter,"S11");
        w.traces[0].scale.per_division=0.;assert!(validate(&[w]).is_err());
    }
}
