//! Mirror the project channel plan without preset or deleting measurements.
use crate::{Error, Result, Session, pna};
use rf_core::{
    Config,
    pna_plan::{Format, Plan},
};
pub fn name(ch: u32, window: usize, trace: usize, parameter: &str) -> String {
    format!("RFW_CH{ch}_W{}_T{}_{parameter}", window + 1, trace + 1)
}
pub fn apply(s: &mut dyn Session, c: &Config, plan: &Plan) -> Result<()> {
    plan.validate(
        c.instrument.port_count,
        c.instrument
            .pna
            .power_sweep
            .then_some(c.instrument.pna.source_port),
    )
    .map_err(Error::Protocol)?;
    if !plan.synchronize_display {
        return Ok(());
    }
    let ch = c.instrument.channel;
    let mut catalog = pna::measurements(&s.query(&format!("CALC{ch}:PAR:CAT:EXT?"))?)?;
    // Reject a name collision before changing any display or measurement.
    for (wi, w) in plan.windows.iter().enumerate() {
        for (ti, t) in w.traces.iter().enumerate() {
            let n = name(ch, wi, ti, &t.parameter);
            if catalog
                .iter()
                .any(|(existing, p)| existing == &n && p != &t.parameter)
            {
                return Err(Error::Protocol(format!(
                    "Nom réservé {n} associé à un autre paramètre ; retirer cette mesure avant de changer le plan"
                )));
            }
        }
    }
    for (wi, w) in plan.windows.iter().enumerate() {
        let win = plan.first_window + wi as u32;
        s.write(&format!("DISP:WIND{win}:STAT ON"))?;
        // These window IDs are explicitly selected in the editor. Removing a
        // displayed trace does not delete its underlying channel measurement.
        for id in pna::list(&s.query(&format!("DISP:WIND{win}:CAT?"))?) {
            if let Ok(id) = id.parse::<u32>() {
                s.write(&format!("DISP:WIND{win}:TRAC{id}:DEL"))?;
            }
        }
        s.write(&format!("DISP:WIND{win}:TRAC:Y:COUP OFF"))?;
        for (ti, t) in w.traces.iter().enumerate() {
            let n = name(ch, wi, ti, &t.parameter);
            let trace = ti + 1;
            if !catalog.iter().any(|(existing, _)| existing == &n) {
                let param = t.parameter.replace('_', ",");
                s.write(&format!("CALC{ch}:PAR:DEF:EXT \"{n}\",\"{param}\""))?;
                catalog.push((n.clone(), t.parameter.clone()));
            }
            s.write(&format!("CALC{ch}:PAR:SEL \"{n}\",FAST"))?;
            s.write(&format!("CALC{ch}:FORM {}", t.format.scpi()))?;
            if !t.parameter.starts_with('S') {
                s.write(&format!("CALC{ch}:FORM:UNIT MLOG,DBM"))?;
            }
            s.write(&format!("DISP:WIND{win}:TRAC{trace}:FEED \"{n}\""))?;
            if t.automatic {
                s.write(&format!("DISP:WIND{win}:TRAC{trace}:Y:AUTO"))?;
            } else if t.format != Format::Smith {
                s.write(&format!(
                    "DISP:WIND{win}:TRAC{trace}:Y:PDIV {}",
                    t.per_division
                ))?;
                s.write(&format!(
                    "DISP:WIND{win}:TRAC{trace}:Y:RLEV {}",
                    t.reference
                ))?;
                s.write(&format!(
                    "DISP:WIND{win}:TRAC{trace}:Y:RPOS {}",
                    t.reference_position
                ))?;
            }
        }
        s.write(&format!(
            "CALC{ch}:PAR:SEL \"{}\",FAST",
            name(ch, wi, w.active_trace, &w.traces[w.active_trace].parameter)
        ))?;
    }
    // TILE preserves the explicit feeds; QUAD/OVERLAY rearrange all measures.
    s.write("DISP:ARR TILE")?;
    pna::check_error(s, "Écran PNA")
}
