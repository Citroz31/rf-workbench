use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
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
        })
    {
        return Err("Fenêtres de résultats invalides".into());
    }
    Ok(())
}
