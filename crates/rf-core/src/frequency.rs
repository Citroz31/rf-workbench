use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Unit {
    Hz,
    Khz,
    Mhz,
    #[default]
    Ghz,
}
impl Unit {
    pub const ALL: [Self; 4] = [Self::Hz, Self::Khz, Self::Mhz, Self::Ghz];
    pub fn factor(self) -> f64 {
        match self { Self::Hz => 1., Self::Khz => 1e3, Self::Mhz => 1e6, Self::Ghz => 1e9 }
    }
    pub fn label(self) -> &'static str {
        match self { Self::Hz => "Hz", Self::Khz => "kHz", Self::Mhz => "MHz", Self::Ghz => "GHz" }
    }
}
/// Keep endpoints; choose enough intervals to never exceed the requested step.
pub fn points_for_step(start: f64, stop: f64, step: f64) -> Option<usize> {
    if !start.is_finite() || !stop.is_finite() || !step.is_finite() || start <= 0. || stop <= start || step <= 0. { return None; }
    let intervals = (stop - start) / step;
    // Avoid an extra point for rounding noise at integral interval counts.
    let rounded = intervals.round();
    let intervals = if (intervals - rounded).abs() <= 1e-10 * intervals.max(1.) { rounded } else { intervals.ceil() };
    Some((intervals as usize).saturating_add(1).clamp(2, crate::MAX_POINTS))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn units_and_precise_steps() {
        assert_eq!(2.5 * Unit::Mhz.factor(), 2_500_000.);
        assert_eq!(points_for_step(36e9, 38e9, 5e6), Some(401));
        assert_eq!(points_for_step(1., 1.3, 0.1), Some(4));
        assert_eq!(points_for_step(1e9, 1.01e9, 3e6), Some(5));
        assert_eq!(points_for_step(1., 2., 0.), None);
        assert_eq!(points_for_step(1., 2., 1e-30), Some(crate::MAX_POINTS));
    }
}
