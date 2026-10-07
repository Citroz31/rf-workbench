//! Deliberately ideal demonstration models; no manufacturer accuracy is claimed.
use crate::{Result, RunResult, Value};
use rf_core::{Kind, Measurement, NetworkTrace, Node, Waveform};
use rf_instruments::Resource;
use std::collections::BTreeMap;

fn quantize(x: f64, bits: u8) -> f64 {
    let steps = (1_u64 << bits) as f64 - 1.;
    ((x.clamp(-1., 1.) + 1.) * 0.5 * steps).round() / steps * 2. - 1.
}
pub(crate) fn execute(
    node: &Node,
    inputs: &BTreeMap<usize, Value>,
    result: &mut RunResult,
) -> Result<Vec<Value>> {
    let c = &node.config;
    if Resource::parse(&c.resource).map_err(|e| e.to_string())? != Resource::Sim {
        return Err(format!(
            "{} : profil matériel non implémenté ; utiliser la console SCPI ou Python pour un pilote validé",
            node.title
        ));
    }
    let values = match node.kind {
        Kind::Awg => {
            let waveform = |phase: f64| Waveform {
                samples: (0..c.samples)
                    .map(|i| {
                        quantize(
                            (std::f64::consts::TAU * c.tone_hz * i as f64 / c.sample_rate_hz
                                + phase)
                                .sin(),
                            c.resolution_bits,
                        )
                    })
                    .collect(),
                sample_rate_hz: c.sample_rate_hz,
                tone_hz: c.tone_hz,
                unit: "FS".into(),
                simulated: true,
            };
            let i = waveform(0.);
            result.waveform = Some(i.clone());
            vec![
                Value::Digital(i),
                Value::Digital(waveform(std::f64::consts::FRAC_PI_2)),
            ]
        }
        Kind::Dac => {
            let Some(Value::Digital(w)) = inputs.get(&0) else {
                return Err("DAC : échantillons numériques requis".into());
            };
            let mut w = w.clone();
            w.unit = "V".into();
            w.samples
                .iter_mut()
                .for_each(|s| *s = quantize(*s, c.resolution_bits) * c.voltage_v);
            result.waveform = Some(w.clone());
            vec![Value::Analog(w)]
        }
        Kind::Adc => {
            let Some(Value::Analog(w)) = inputs.get(&0) else {
                return Err("CAN : tension analogique requise".into());
            };
            let mut w = w.clone();
            w.unit = "FS".into();
            w.samples
                .iter_mut()
                .for_each(|s| *s = quantize(*s / c.voltage_v, c.resolution_bits));
            result.waveform = Some(w.clone());
            vec![Value::Digital(w)]
        }
        Kind::IqModulator => {
            let (
                Some(Value::Analog(i)),
                Some(Value::Analog(q)),
                Some(Value::Signal {
                    frequency,
                    level,
                    simulated,
                }),
            ) = (inputs.get(&0), inputs.get(&1), inputs.get(&2))
            else {
                return Err("I/Q : relier I, Q et LO".into());
            };
            if !simulated || !i.simulated || !q.simulated {
                return Err("Le modèle I/Q est limité à la simulation".into());
            }
            if i.samples.len() != q.samples.len()
                || i.sample_rate_hz != q.sample_rate_hz
                || i.tone_hz != q.tone_hz
            {
                return Err("I/Q : mêmes longueur, cadence et fréquence de base requises".into());
            }
            let power = i
                .samples
                .iter()
                .zip(&q.samples)
                .map(|(i, q)| i * i + q * q)
                .sum::<f64>()
                / i.samples.len() as f64;
            vec![Value::Signal {
                frequency: frequency + i.tone_hz,
                level: (level + 10. * power.max(1e-16).log10() - c.loss_db).clamp(-160., 30.),
                simulated: true,
            }]
        }
        Kind::Pna | Kind::PnaX => {
            let loss = match inputs.get(&0) {
                Some(Value::Dut { loss, .. }) => *loss,
                _ => c.loss_db,
            };
            let reflection = matches!(c.s_parameter.as_str(), "S11" | "S22");
            let frequency_hz: Vec<_> = (0..c.points)
                .map(|i| c.start_hz + (c.stop_hz - c.start_hz) * i as f64 / (c.points - 1) as f64)
                .collect();
            let network = NetworkTrace {
                magnitude_db: frequency_hz
                    .iter()
                    .map(|f| {
                        if reflection {
                            -20. + 0.7
                                * ((*f - c.start_hz) / (c.stop_hz - c.start_hz)
                                    * std::f64::consts::TAU)
                                    .cos()
                        } else {
                            -loss
                                + 0.15
                                    * ((*f - c.start_hz) / (c.stop_hz - c.start_hz)
                                        * std::f64::consts::TAU)
                                        .sin()
                        }
                    })
                    .collect(),
                phase_deg: frequency_hz.iter().map(|f| -360. * f * 1e-9).collect(),
                frequency_hz,
                parameter: c.s_parameter.clone(),
                simulated: true,
            };
            result.network = Some(network.clone());
            vec![Value::Network]
        }
        Kind::VariableResistor => vec![Value::Scalar(c.resistance_ohm)],
        Kind::Thermostream => vec![Value::Scalar(c.temperature_c)],
        Kind::Thermometer => vec![Value::Scalar(match inputs.get(&0) {
            Some(Value::Scalar(t)) => *t,
            _ => c.temperature_c,
        })],
        Kind::NoiseFigureMeter => vec![Value::Scalar(match inputs.get(&0) {
            Some(Value::Dut { noise, .. }) => *noise,
            _ => c.noise_figure_db,
        })],
        Kind::PowerSensor => {
            let Some(Value::Signal {
                level, simulated, ..
            }) = inputs.get(&0)
            else {
                return Err("Power Sensor : signal RF requis".into());
            };
            if !simulated {
                return Err("Power Sensor simulé ne mesure pas un signal matériel".into());
            }
            vec![Value::Scalar(*level)]
        }
        Kind::PowerMeter => {
            let Some(Value::Scalar(power)) = inputs.get(&0) else {
                return Err("Power Meter : relier un capteur de puissance".into());
            };
            vec![Value::Scalar(*power)]
        }
        _ => return Err("Modèle de simulation absent".into()),
    };
    for (v, p) in values.iter().zip(node.kind.outputs()) {
        if let Value::Scalar(value) = v {
            result.measurements.push(Measurement {
                name: node.title.clone(),
                value: *value,
                unit: p.port.label().into(),
                simulated: true,
            });
        }
    }
    Ok(values)
}
