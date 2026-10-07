//! Bounded previews from the actual execution buffers, never fabricated UI data.
use crate::{RunResult, Value};
use rf_core::{NetworkTrace, Node, Trace, Waveform};
pub const PREVIEW_SAMPLES: usize = 4096;
#[derive(Clone, Debug)]
pub enum BufferData {
    Dsp(rf_core::dsp::Data),
    Signal {
        frequency_hz: f64,
        level_dbm: f64,
        simulated: bool,
    },
    Trace(Trace),
    Waveform(Waveform),
    Network(NetworkTrace),
    Scalar {
        value: f64,
        unit: String,
        simulated: bool,
    },
    Dut {
        loss_db: f64,
        noise_figure_db: f64,
    },
}
#[derive(Clone, Debug)]
pub struct Buffer {
    pub node: u64,
    pub port: usize,
    pub name: String,
    pub total_samples: usize,
    pub data: std::sync::Arc<BufferData>,
    pub probe: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Before,
    After,
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub node: u64,
    pub title: String,
    pub paused: bool,
    pub completed: Vec<u64>,
    pub buffers: Vec<Buffer>,
}
impl Buffer {
    pub(crate) fn capture(node: &Node, port: usize, value: &Value, _result: &RunResult) -> Self {
        let mut total_samples = 1;
        let data = match value {
            Value::Dsp(d) => {
                let mut d = d.as_ref().clone();
                match &mut d {
                    rf_core::dsp::Data::Iq(f) => {
                        total_samples = f.samples.len();
                        f.samples.truncate(PREVIEW_SAMPLES);
                    }
                    rf_core::dsp::Data::Bits(f) => {
                        total_samples = f.bits.len();
                        f.bits.truncate(PREVIEW_SAMPLES);
                        f.llr.truncate(PREVIEW_SAMPLES);
                    }
                    rf_core::dsp::Data::Spectrum(f) => {
                        total_samples = f.levels.len();
                        f.levels.truncate(PREVIEW_SAMPLES);
                        f.frequency_hz.truncate(PREVIEW_SAMPLES);
                    }
                    _ => {}
                }
                BufferData::Dsp(d)
            }
            Value::Signal {
                frequency,
                level,
                simulated,
            } => BufferData::Signal {
                frequency_hz: *frequency,
                level_dbm: *level,
                simulated: *simulated,
            },
            Value::Trace(t) => {
                total_samples = t.amplitude_dbm.len();
                BufferData::Trace(Trace {
                    frequency_hz: t
                        .frequency_hz
                        .iter()
                        .take(PREVIEW_SAMPLES)
                        .copied()
                        .collect(),
                    amplitude_dbm: t
                        .amplitude_dbm
                        .iter()
                        .take(PREVIEW_SAMPLES)
                        .copied()
                        .collect(),
                    simulated: t.simulated,
                })
            }
            Value::Digital(w) | Value::Analog(w) => {
                total_samples = w.samples.len();
                BufferData::Waveform(Waveform {
                    samples: w.samples.iter().take(PREVIEW_SAMPLES).copied().collect(),
                    sample_rate_hz: w.sample_rate_hz,
                    tone_hz: w.tone_hz,
                    unit: w.unit.clone(),
                    simulated: w.simulated,
                })
            }
            Value::Network(trace) => {
                let mut t = trace.clone();
                total_samples = t.frequency_hz.len();
                t.frequency_hz.truncate(PREVIEW_SAMPLES);
                t.magnitude_db.truncate(PREVIEW_SAMPLES);
                t.phase_deg.truncate(PREVIEW_SAMPLES);
                BufferData::Network(t)
            }
            Value::Scalar(value, simulated) => BufferData::Scalar {
                value: *value,
                unit: node.kind.outputs()[port].port.label().into(),
                simulated: *simulated,
            },
            Value::Dut { loss, noise, .. } => BufferData::Dut {
                loss_db: *loss,
                noise_figure_db: *noise,
            },
        };
        Self {
            node: node.id,
            port,
            name: format!("{} / {}", node.title, node.kind.outputs()[port].name),
            total_samples,
            data: std::sync::Arc::new(data),
            probe: node.probe,
        }
    }
}
