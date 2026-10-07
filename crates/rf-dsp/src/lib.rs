//! Bounded, deterministic baseband processors. No instrument access here.
pub mod fec;
pub mod modem;
use rf_core::dsp::*;
use std::{collections::BTreeMap, f64::consts::PI};
pub type Result<T> = std::result::Result<T, String>;
#[derive(Default)]
struct State {
    settings: Option<Settings>,
    last: Option<(u64, f64, String)>,
    x: Vec<Complex>,
    y: Vec<Complex>,
    phase: f64,
    freq: f64,
    power: f64,
}
#[derive(Default)]
pub struct Processor {
    states: BTreeMap<u64, State>,
}
impl Processor {
    pub fn reset(&mut self) {
        self.states.clear()
    }
    pub fn process(
        &mut self,
        id: u64,
        op: Op,
        c: &Settings,
        input: &[Data],
        sequence: u64,
    ) -> Result<Data> {
        c.validate()?;
        for d in input {
            d.validate()?
        }
        let state = self.states.entry(id).or_default();
        if state.settings.as_ref() != Some(c) {
            *state = State::default();
            state.settings = Some(c.clone())
        }
        if let Some(Data::Iq(f)) = input.first() {
            if f.time.discontinuity
                || state.last.as_ref().is_some_and(|(end, rate, clock)| {
                    *end != f.time.first_sample
                        || *rate != f.sample_rate
                        || *clock != f.time.clock_domain
                })
            {
                state.x.clear();
                state.y.clear();
                state.phase = 0.;
                state.freq = 0.;
                state.power = 0.
            }
            state.last = Some((
                f.time.first_sample + f.samples.len() as u64,
                f.sample_rate,
                f.time.clock_domain.clone(),
            ));
        }
        let iq = || -> Result<IqFrame> {
            match input.first() {
                Some(Data::Iq(f)) => Ok(f.clone()),
                _ => Err("Entrée I/Q requise".into()),
            }
        };
        let bits = || -> Result<BitFrame> {
            match input.first() {
                Some(Data::Bits(f)) => Ok(f.clone()),
                _ => Err("Entrée bits requise".into()),
            }
        };
        let result = match op {
            Op::IqSource => {
                if c.tone_hz.abs() >= c.rate / 2. {
                    return Err("Source : fréquence hors Nyquist".into());
                }
                let first = sequence
                    .checked_mul(c.samples as u64)
                    .ok_or("Index temporel trop grand")?;
                Data::Iq(IqFrame {
                    samples: (0..c.samples)
                        .map(|i| {
                            Complex::polar(
                                c.amplitude,
                                2. * PI * c.tone_hz * (first + i as u64) as f64 / c.rate,
                            )
                        })
                        .collect(),
                    sample_rate: c.rate,
                    center_hz: c.center_hz,
                    unit: c.source_unit,
                    simulated: true,
                    time: TimeTag {
                        first_sample: first,
                        ..Default::default()
                    },
                })
            }
            Op::BitSource => {
                let mut rng = Rng(c.seed.wrapping_add(sequence));
                Data::Bits(BitFrame {
                    bits: (0..c.samples).map(|_| (rng.next_u64() & 1) as u8).collect(),
                    llr: vec![],
                    simulated: true,
                })
            }
            Op::Fir | Op::Iir | Op::Decimate | Op::Interpolate => {
                let mut f = iq()?;
                let original_first = f.time.first_sample;
                if op == Op::Interpolate {
                    if f.samples.len() * c.factor > 65536 {
                        return Err("Interpolation trop volumineuse".into());
                    }
                    let mut expanded = vec![Complex::default(); f.samples.len() * c.factor];
                    for (i, z) in f.samples.iter().enumerate() {
                        expanded[i * c.factor] = *z * c.factor as f64
                    }
                    f.samples = expanded;
                    f.sample_rate *= c.factor as f64;
                    f.time.first_sample = f
                        .time
                        .first_sample
                        .checked_mul(c.factor as u64)
                        .ok_or("Index interpolation trop grand")?;
                }
                let mut out = Vec::with_capacity(f.samples.len());
                for z in &f.samples {
                    state.x.insert(0, *z);
                    state.x.truncate(c.taps.len());
                    let mut value = state
                        .x
                        .iter()
                        .zip(&c.taps)
                        .fold(Complex::default(), |v, (x, t)| v + *x * *t);
                    if op == Op::Iir {
                        for (y, a) in state.y.iter().zip(c.denominator.iter().skip(1)) {
                            value = value - *y * *a;
                        }
                        value = value / c.denominator[0];
                        state.y.insert(0, value);
                        state.y.truncate(c.denominator.len() - 1);
                    }
                    out.push(value)
                }
                if op == Op::Decimate {
                    let phase = ((c.factor as u64 - original_first % c.factor as u64)
                        % c.factor as u64) as usize;
                    out = out.into_iter().skip(phase).step_by(c.factor).collect();
                    f.sample_rate /= c.factor as f64;
                    f.time.first_sample = (original_first + phase as u64) / c.factor as u64;
                }
                f.samples = out;
                Data::Iq(f)
            }
            Op::Window => {
                let mut f = iq()?;
                let n = f.samples.len();
                for (i, z) in f.samples.iter_mut().enumerate() {
                    *z = *z * window(&c.window, i, n)?
                }
                Data::Iq(f)
            }
            Op::Fft | Op::Psd | Op::Spectrogram => Data::Spectrum(psd(&iq()?, c, op == Op::Fft)?),
            Op::Agc => {
                let mut f = iq()?;
                for z in &mut f.samples {
                    state.power = (1. - c.bandwidth) * state.power + c.bandwidth * z.norm2();
                    let gain = (c.amplitude / state.power.max(1e-20).sqrt()).clamp(1e-6, 1e6);
                    *z = *z * gain
                }
                Data::Iq(f)
            }
            Op::Pll | Op::Costas => {
                if op == Op::Costas && !matches!(c.order, 2 | 4) {
                    return Err("Costas : BPSK/QPSK uniquement, ordre 2 ou 4".into());
                }
                let mut f = iq()?;
                for z in &mut f.samples {
                    let corrected = *z * Complex::polar(1., -state.phase);
                    let error = if op == Op::Pll {
                        corrected.arg()
                    } else if c.order == 2 {
                        corrected.re.signum() * corrected.im
                    } else {
                        corrected.re.signum() * corrected.im - corrected.im.signum() * corrected.re
                    };
                    state.freq = (state.freq + c.bandwidth * c.bandwidth * error).clamp(-PI, PI);
                    state.phase =
                        (state.phase + state.freq + 2. * c.bandwidth * error).rem_euclid(2. * PI);
                    *z = corrected;
                }
                Data::Iq(f)
            }
            Op::SymbolTiming => {
                let mut f = iq()?;
                if c.sps < 2 {
                    return Err("Gardner nécessite au moins 2 samples/symbole".into());
                }
                let mut position = c.sps as f64;
                let mut result = vec![];
                let interp = |p: f64| {
                    let i = p.floor() as usize;
                    f.samples[i] * (1. - p.fract()) + f.samples[i + 1] * p.fract()
                };
                while position + c.sps as f64 / 2. + 1. < f.samples.len() as f64 {
                    let early = interp(position - c.sps as f64 / 2.);
                    let late = interp(position + c.sps as f64 / 2.);
                    let mid = interp(position);
                    let error = ((late - early) * mid.conj()).re;
                    result.push(mid);
                    position += c.sps as f64 + (c.bandwidth * error).clamp(-0.2, 0.2);
                }
                f.samples = result;
                f.sample_rate /= c.sps as f64;
                f.time.first_sample /= c.sps as u64;
                f.time.discontinuity = true;
                Data::Iq(f)
            }
            Op::FrameSync => {
                let mut f = bits()?;
                let found = f
                    .bits
                    .windows(c.preamble.len())
                    .position(|w| {
                        w == c.preamble.as_slice()
                            || w.iter().zip(&c.preamble).all(|(a, b)| *a == 1 - *b)
                    })
                    .ok_or("Préambule absent")?;
                let invert = f.bits[found] != c.preamble[0];
                let start = found + c.preamble.len();
                if start + c.packet_bits > f.bits.len() {
                    return Err("Trame incomplète après préambule".into());
                }
                f.bits = f.bits[start..start + c.packet_bits]
                    .iter()
                    .map(|b| if invert { 1 - *b } else { *b })
                    .collect();
                f.llr.clear();
                Data::Bits(f)
            }
            Op::DigitalMod => {
                let mut f = modem::modulate(&bits()?, c)?;
                f.time.first_sample = sequence
                    .checked_mul(f.samples.len() as u64)
                    .ok_or("Index temporel trop grand")?;
                Data::Iq(f)
            }
            Op::DigitalDemod => Data::Bits(modem::demodulate(&iq()?, c)?),
            Op::AnalogMod | Op::AnalogDemod => {
                let mut f = iq()?;
                if !["AM", "FM", "PM"].contains(&c.modulation.as_str()) {
                    return Err("Choisir AM, FM ou PM".into());
                }
                for z in &mut f.samples {
                    let v = z.re;
                    if op == Op::AnalogMod {
                        *z = match c.modulation.as_str() {
                            "AM" => Complex::new(1. + c.amplitude * v, 0.),
                            "FM" => {
                                state.phase += 2. * PI * c.tone_hz * v / f.sample_rate;
                                Complex::polar(1., state.phase)
                            }
                            _ => Complex::polar(1., c.amplitude * v),
                        };
                    } else {
                        let p = z.arg();
                        let v = match c.modulation.as_str() {
                            "AM" => (z.norm2().sqrt() - 1.) / c.amplitude,
                            "FM" => {
                                let d = wrap(p - state.phase);
                                state.phase = p;
                                d * f.sample_rate / (2. * PI * c.tone_hz)
                            }
                            _ => p / c.amplitude,
                        };
                        *z = Complex::new(v, 0.);
                    }
                }
                Data::Iq(f)
            }
            Op::ConvEncode
            | Op::ConvDecode
            | Op::RsEncode
            | Op::RsDecode
            | Op::LdpcEncode
            | Op::LdpcDecode
            | Op::TurboEncode
            | Op::TurboDecode => Data::Bits(fec::process(op, &bits()?, c)?),
            Op::Channel => {
                let mut f = iq()?;
                let power =
                    f.samples.iter().map(|z| z.norm2()).sum::<f64>() / f.samples.len() as f64;
                let sigma = (power * 10f64.powf(-c.noise_db / 10.) / 2.).sqrt();
                let mut rng = Rng(c.seed.wrapping_add(sequence));
                let fading = if c.fading {
                    Complex::new(rng.normal(), rng.normal()) / 2f64.sqrt()
                } else {
                    Complex::new(1., 0.)
                };
                for (i, z) in f.samples.iter_mut().enumerate() {
                    state.x.insert(0, *z);
                    state.x.truncate(c.multipath.len());
                    let filtered = state
                        .x
                        .iter()
                        .zip(&c.multipath)
                        .fold(Complex::default(), |v, (x, t)| v + *x * *t);
                    let nonlinear = filtered / (1. + c.nonlinearity * filtered.norm2());
                    let p = c.phase
                        + 2. * PI * c.frequency_offset * (f.time.first_sample + i as u64) as f64
                            / f.sample_rate;
                    *z = nonlinear * fading * Complex::polar(1., p)
                        + Complex::new(sigma * rng.normal(), sigma * rng.normal());
                }
                f.simulated = true;
                Data::Iq(f)
            }
            Op::DcOffset | Op::IqBalance | Op::Cable | Op::CalibrationTable => {
                let mut f = iq()?;
                let dc = if op == Op::DcOffset {
                    f.samples
                        .iter()
                        .copied()
                        .fold(Complex::default(), |a, b| a + b)
                        / f.samples.len() as f64
                } else {
                    c.dc
                };
                let factor = if op == Op::CalibrationTable {
                    let row = table(&c.calibration, f.center_hz)?;
                    Complex::polar(10f64.powf(row[0] / 20.), row[1] * PI / 180.)
                } else {
                    Complex::polar(
                        10f64.powf(c.cable_db / 20.),
                        2. * PI * f.center_hz * c.delay_s,
                    )
                };
                for z in &mut f.samples {
                    *z = match op {
                        Op::DcOffset => *z - dc,
                        Op::IqBalance => {
                            let a = *z - dc;
                            Complex::new(
                                a.re,
                                (a.im / c.iq_gain - a.re * c.iq_phase.sin()) / c.iq_phase.cos(),
                            )
                        }
                        _ => *z * factor,
                    };
                }
                if op == Op::CalibrationTable
                    && f.unit == Unit::Fs
                    && let Some(volts) = c.volts_per_fs
                {
                    for z in &mut f.samples {
                        *z = *z * volts;
                    }
                    f.unit = Unit::Volt;
                }
                Data::Iq(f)
            }
            Op::Power => {
                let f = iq()?;
                let rms = (f.samples.iter().map(|z| z.norm2()).sum::<f64>()
                    / f.samples.len() as f64)
                    .sqrt();
                if f.unit != Unit::Volt {
                    return Err("Puissance dBm : amplitude en volts et impédance requises".into());
                }
                quantity(
                    convert(rms, Unit::Volt, Unit::Dbm, c.impedance)?,
                    Unit::Dbm,
                    f.simulated,
                )
            }
            Op::Evm | Op::Snr => {
                let f = iq()?;
                let Some(Data::Iq(reference)) = input.get(1) else {
                    return Err("Référence I/Q requise".into());
                };
                if f.samples.len() != reference.samples.len()
                    || f.sample_rate != reference.sample_rate
                    || f.unit != reference.unit
                    || f.time.first_sample != reference.time.first_sample
                    || f.center_hz != reference.center_hz
                    || f.time.clock_domain != reference.time.clock_domain
                    || f.time.epoch_ns != reference.time.epoch_ns
                {
                    return Err("Référence non alignée : taille/cadence/unité/timestamp".into());
                }
                let signal = reference.samples.iter().map(|z| z.norm2()).sum::<f64>();
                let error = f
                    .samples
                    .iter()
                    .zip(&reference.samples)
                    .map(|(a, b)| (*a - *b).norm2())
                    .sum::<f64>();
                if signal <= 0. {
                    return Err("Référence nulle".into());
                }
                let (value, unit) = if op == Op::Evm {
                    (100. * (error / signal).sqrt(), Unit::Percent)
                } else {
                    if error <= 0. {
                        return Err("SNR infini : erreur nulle".into());
                    }
                    (10. * (signal / error).log10(), Unit::Db)
                };
                quantity(value, unit, f.simulated || reference.simulated)
            }
            Op::Ber | Op::Per => {
                let f = bits()?;
                let Some(Data::Bits(r)) = input.get(1) else {
                    return Err("Référence bits requise".into());
                };
                if f.bits.len() != r.bits.len() {
                    return Err("Longueurs BER/PER différentes".into());
                }
                let errors = if op == Op::Ber {
                    f.bits.iter().zip(&r.bits).filter(|(a, b)| a != b).count()
                } else {
                    if f.bits.len() % c.packet_bits != 0 {
                        return Err("PER : paquet partiel".into());
                    }
                    f.bits
                        .chunks(c.packet_bits)
                        .zip(r.bits.chunks(c.packet_bits))
                        .filter(|(a, b)| a != b)
                        .count()
                };
                let count = if op == Op::Ber {
                    f.bits.len()
                } else {
                    f.bits.len() / c.packet_bits
                };
                quantity(
                    errors as f64 / count as f64,
                    Unit::Ratio,
                    f.simulated || r.simulated,
                )
            }
            Op::Thd => {
                let f = iq()?;
                if c.tone_hz <= 0. {
                    return Err("THD : fondamentale positive requise".into());
                }
                let amplitude = |hz: f64| {
                    f.samples
                        .iter()
                        .enumerate()
                        .fold(Complex::default(), |a, (i, z)| {
                            a + *z * Complex::polar(1., -2. * PI * hz * i as f64 / f.sample_rate)
                        })
                        .norm2()
                };
                let fund = amplitude(c.tone_hz);
                if fund <= 1e-20 {
                    return Err("Fondamentale absente".into());
                }
                let harmonic = (2..=10)
                    .take_while(|n| c.tone_hz * (*n as f64) < f.sample_rate / 2.)
                    .map(|n| amplitude(c.tone_hz * n as f64))
                    .sum::<f64>();
                quantity(100. * (harmonic / fund).sqrt(), Unit::Percent, f.simulated)
            }
            Op::PhaseNoise => {
                let mut f = iq()?;
                let mut phase = vec![];
                let mut last = 0.;
                let mut unwrapped = 0.;
                for z in &f.samples {
                    if z.norm2() < 1e-20 {
                        return Err("Bruit de phase : amplitude nulle".into());
                    }
                    let p = z.arg();
                    unwrapped += wrap(p - last);
                    phase.push(unwrapped);
                    last = p;
                }
                let n = phase.len() as f64;
                let mean = phase.iter().sum::<f64>() / n;
                let center = (n - 1.) / 2.;
                let slope = phase
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (i as f64 - center) * (p - mean))
                    .sum::<f64>()
                    / (0..phase.len())
                        .map(|i| (i as f64 - center).powi(2))
                        .sum::<f64>();
                f.samples = phase
                    .iter()
                    .enumerate()
                    .map(|(i, p)| Complex::new(p - mean - slope * (i as f64 - center), 0.))
                    .collect();
                f.center_hz = 0.;
                let mut s = psd(&f, c, false)?;
                let pairs: Vec<_> = s
                    .frequency_hz
                    .iter()
                    .zip(&s.levels)
                    .filter(|(f, _)| **f > 0.)
                    .map(|(f, p)| (*f, *p))
                    .collect();
                s.frequency_hz = pairs.iter().map(|p| p.0).collect();
                s.levels = pairs.iter().map(|p| p.1).collect();
                s.unit = Unit::DbcPerHz;
                Data::Spectrum(s)
            }
            Op::SpectralPeak => {
                let Some(Data::Spectrum(s)) = input.first() else {
                    return Err("Spectre requis".into());
                };
                let peak = s
                    .levels
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .ok_or("Spectre vide")?
                    .0;
                quantity(s.frequency_hz[peak], Unit::Hz, s.simulated)
            }
            Op::Convert | Op::Uncertainty => {
                let Some(Data::Quantity {
                    value,
                    unit,
                    simulated,
                }) = input.first()
                else {
                    return Err("Quantité typée requise".into());
                };
                if op == Op::Convert {
                    quantity(
                        convert(*value, *unit, c.target_unit, c.impedance)?,
                        c.target_unit,
                        *simulated,
                    )
                } else {
                    quantity(
                        c.coverage * c.uncertainties.iter().map(|u| u * u).sum::<f64>().sqrt(),
                        *unit,
                        *simulated,
                    )
                }
            }
            Op::IoSource | Op::IoSink | Op::Vswr => {
                return Err("Opération HAL/VNA exécutée par le runtime".into());
            }
        };
        result.validate()?;
        Ok(result)
    }
}
fn quantity(value: f64, unit: Unit, simulated: bool) -> Data {
    Data::Quantity {
        value,
        unit,
        simulated,
    }
}
pub fn self_tests() -> Vec<rf_core::TestResult> {
    let c = Settings::default();
    let mut p = Processor::default();
    let source = p.process(1, Op::IqSource, &c, &[], 0);
    let density = source
        .as_ref()
        .ok()
        .and_then(|f| p.process(2, Op::Psd, &c, std::slice::from_ref(f), 0).ok());
    let psd_ok = if let Some(Data::Spectrum(s)) = density {
        let power =
            s.levels.iter().map(|x| 10f64.powf(x / 10.)).sum::<f64>() * c.rate / c.fft_size as f64;
        (power - 1.).abs() < 1e-6
    } else {
        false
    };
    let b = BitFrame {
        bits: (0..1024).map(|i| (i % 2) as u8).collect(),
        llr: vec![],
        simulated: true,
    };
    let modem_ok = modem::modulate(&b, &c)
        .and_then(|f| modem::demodulate(&f, &c))
        .is_ok_and(|d| d.bits == b.bits);
    let original: Vec<u8> = (0..239).map(|i| i as u8).collect();
    let rs_ok = fec::rs_encode(&original, 16).is_ok_and(|mut encoded| {
        for i in 0..8 {
            encoded[i * 23] ^= 0x55;
        }
        fec::rs_decode(&mut encoded, 16).is_ok_and(|n| n == 8 && encoded[..239] == original)
    });
    let conv_ok = fec::process(Op::ConvEncode, &b, &c)
        .and_then(|f| fec::process(Op::ConvDecode, &f, &c))
        .is_ok_and(|f| f.bits == b.bits);
    let unit_ok =
        convert(0., Unit::Dbm, Unit::Volt, 50.).is_ok_and(|v| (v - 0.22360679775).abs() < 1e-10);
    [
        (
            "PSD / puissance intégrée",
            psd_ok,
            "Welch normalisé par Fs et énergie de fenêtre",
        ),
        (
            "QAM16 / bits",
            modem_ok,
            "Constellation Gray et décision max-log",
        ),
        (
            "Reed-Solomon / 8 erreurs",
            rs_ok,
            "RS(255,239), correction de 8 octets",
        ),
        (
            "Convolutionnel / Viterbi",
            conv_ok,
            "K=3, rate 1/2 et terminaison zéro",
        ),
        ("Unités RMS / 50 ohms", unit_ok, "0 dBm = 223.6068 mV RMS"),
    ]
    .into_iter()
    .map(|(name, passed, detail)| rf_core::TestResult {
        name: name.into(),
        passed,
        detail: detail.into(),
    })
    .collect()
}
pub fn wrap(p: f64) -> f64 {
    (p + PI).rem_euclid(2. * PI) - PI
}
pub fn window(kind: &str, i: usize, n: usize) -> Result<f64> {
    let p = 2. * PI * i as f64 / (n.max(2) - 1) as f64;
    match kind {
        "Rectangular" => Ok(1.),
        "Hann" => Ok(0.5 - 0.5 * p.cos()),
        "Hamming" => Ok(0.54 - 0.46 * p.cos()),
        "Blackman" => Ok(0.42 - 0.5 * p.cos() + 0.08 * (2. * p).cos()),
        _ => Err("Fenêtre inconnue".into()),
    }
}
pub fn fft(x: &mut [Complex], inverse: bool) -> Result<()> {
    let n = x.len();
    if !n.is_power_of_two() || n > 65536 {
        return Err("FFT : longueur puissance de deux ≤ 65536".into());
    }
    if n == 1 {
        return Ok(());
    }
    let bits = n.trailing_zeros();
    for i in 0..n {
        let j = i.reverse_bits() >> (usize::BITS - bits);
        if j > i {
            x.swap(i, j)
        }
    }
    let mut len = 2;
    while len <= n {
        let w = Complex::polar(
            1.,
            if inverse {
                2. * PI / len as f64
            } else {
                -2. * PI / len as f64
            },
        );
        for block in x.chunks_mut(len) {
            let mut twiddle = Complex::new(1., 0.);
            for j in 0..len / 2 {
                let a = block[j];
                let b = block[j + len / 2] * twiddle;
                block[j] = a + b;
                block[j + len / 2] = a - b;
                twiddle = twiddle * w;
            }
        }
        len *= 2;
    }
    if inverse {
        for z in x {
            *z = *z / n as f64
        }
    }
    Ok(())
}
pub fn psd(f: &IqFrame, c: &Settings, per_bin: bool) -> Result<Spectrum> {
    f.validate()?;
    c.validate()?;
    let n = c.fft_size;
    if f.samples.len() < n {
        return Err("Pas assez de samples pour la FFT".into());
    }
    let w: Vec<f64> = (0..n)
        .map(|i| window(&c.window, i, n))
        .collect::<Result<_>>()?;
    let norm = if per_bin {
        (n * n) as f64
    } else {
        f.sample_rate * w.iter().map(|w| w * w).sum::<f64>()
    };
    let mut powers = vec![0.; n];
    let mut count = 0;
    for start in (0..=f.samples.len() - n).step_by(n / 2) {
        let mut block: Vec<_> = f.samples[start..start + n]
            .iter()
            .zip(&w)
            .map(|(z, w)| *z * *w)
            .collect();
        fft(&mut block, false)?;
        for (i, z) in block.iter().enumerate() {
            powers[(i + n / 2) % n] += z.norm2() / norm;
        }
        count += 1;
        if per_bin {
            break;
        }
    }
    Ok(Spectrum {
        frequency_hz: (0..n)
            .map(|i| f.center_hz + (i as f64 - n as f64 / 2.) * f.sample_rate / n as f64)
            .collect(),
        levels: powers
            .iter()
            .map(|p| 10. * (p / count as f64).max(1e-30).log10())
            .collect(),
        unit: if per_bin {
            if f.unit == Unit::Volt {
                Unit::Dbv
            } else {
                Unit::Dbfs
            }
        } else if f.unit == Unit::Volt {
            Unit::DbVolt2PerHz
        } else {
            Unit::DbFs2PerHz
        },
        simulated: f.simulated,
    })
}
fn table(rows: &[[f64; 3]], f: f64) -> Result<[f64; 2]> {
    let pair = rows
        .windows(2)
        .find(|w| f >= w[0][0] && f <= w[1][0])
        .ok_or("Fréquence hors table calibration ; aucune extrapolation")?;
    let t = (f - pair[0][0]) / (pair[1][0] - pair[0][0]);
    Ok([
        pair[0][1] + t * (pair[1][1] - pair[0][1]),
        pair[0][2] + t * (pair[1][2] - pair[0][2]),
    ])
}
pub struct Rng(pub u64);
impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    pub fn normal(&mut self) -> f64 {
        let a = ((self.next_u64() >> 11) as f64 + 1.) / 9007199254740993.;
        let b = (self.next_u64() >> 11) as f64 / 9007199254740992.;
        (-2. * a.ln()).sqrt() * (2. * PI * b).cos()
    }
}
