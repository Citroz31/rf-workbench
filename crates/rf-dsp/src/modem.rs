//! Gray-labelled memoryless modems; rectangular pulses, explicit symbol rate.
use crate::{Result, fft};
use rf_core::dsp::*;
use std::f64::consts::PI;
fn points(c: &Settings) -> Result<Vec<Complex>> {
    let m = c.order;
    if !m.is_power_of_two() {
        return Err("Ordre du modem : puissance de deux".into());
    }
    let gray = |mut x: usize| {
        let mut decoded = x;
        while x > 0 {
            x >>= 1;
            decoded ^= x;
        }
        decoded
    };
    match c.modulation.as_str() {
        "QAM" | "OFDM" => {
            let side = (m as f64).sqrt() as usize;
            if side * side != m {
                return Err("QAM carré : 4, 16, 64 ou 256".into());
            }
            let scale = (2. * (m - 1) as f64 / 3.).sqrt();
            Ok((0..m)
                .map(|i| {
                    Complex::new(
                        2. * gray(i / side) as f64 - (side - 1) as f64,
                        2. * gray(i % side) as f64 - (side - 1) as f64,
                    ) / scale
                })
                .collect())
        }
        "PSK" => Ok((0..m)
            .map(|i| Complex::polar(1., 2. * PI * gray(i) as f64 / m as f64))
            .collect()),
        "ASK" => {
            let scale = ((m - 1) * (2 * m - 1)) as f64 / 6.;
            Ok((0..m)
                .map(|i| Complex::new(gray(i) as f64 / scale.sqrt(), 0.))
                .collect())
        }
        _ => Err("Modem numérique : ASK, FSK, PSK, QAM ou OFDM".into()),
    }
}
pub fn modulate(bits: &BitFrame, c: &Settings) -> Result<IqFrame> {
    c.validate()?;
    bits.validate()?;
    let k = c.order.ilog2() as usize;
    if !c.order.is_power_of_two() || !bits.bits.len().is_multiple_of(k) {
        return Err("Nombre de bits non multiple de log2(M)".into());
    }
    let symbols: Vec<_> = bits
        .bits
        .chunks(k)
        .map(|b| b.iter().fold(0usize, |v, b| (v << 1) | *b as usize))
        .collect();
    let expected = if c.modulation == "OFDM" {
        symbols
            .len()
            .checked_mul(c.fft_size + c.fft_size / 8)
            .and_then(|n| n.checked_div(c.fft_size))
    } else {
        symbols.len().checked_mul(c.sps)
    };
    if expected.is_none_or(|n| n > 65536) {
        return Err("Modem : sortie limitée à 65536 échantillons".into());
    }
    let mut samples = vec![];
    if c.modulation == "FSK" {
        if c.order > 16 || c.tone_hz <= 0. || c.tone_hz * (c.order - 1) as f64 >= c.rate / 2. {
            return Err("FSK : ordre ≤16, fréquences sous Nyquist".into());
        }
        let mut phase = 0.;
        for index in symbols {
            let hz = (2. * index as f64 - (c.order - 1) as f64) * c.tone_hz;
            for _ in 0..c.sps {
                samples.push(Complex::polar(c.amplitude, phase));
                phase += 2. * PI * hz / c.rate;
            }
        }
    } else {
        let constellation = points(c)?;
        let z: Vec<_> = symbols
            .iter()
            .map(|i| constellation[*i] * c.amplitude)
            .collect();
        if c.modulation == "OFDM" {
            let n = c.fft_size;
            if z.len() % n != 0 {
                return Err("OFDM : compléter un symbole FFT entier".into());
            }
            let cp = n / 8;
            for carriers in z.chunks(n) {
                let mut block = carriers.to_vec();
                fft(&mut block, true)?;
                for x in &mut block {
                    *x = *x * (n as f64).sqrt()
                }
                samples.extend_from_slice(&block[n - cp..]);
                samples.extend(block);
            }
        } else {
            for x in z {
                samples.extend(std::iter::repeat_n(x, c.sps));
            }
        }
    }
    let f = IqFrame {
        samples,
        sample_rate: c.rate,
        center_hz: c.center_hz,
        unit: Unit::Volt,
        simulated: bits.simulated,
        time: TimeTag::default(),
    };
    f.validate()?;
    Ok(f)
}
pub fn demodulate(f: &IqFrame, c: &Settings) -> Result<BitFrame> {
    c.validate()?;
    if !c.order.is_power_of_two() {
        return Err("Ordre modem invalide".into());
    }
    f.validate()?;
    let k = c.order.ilog2() as usize;
    let mut bits = vec![];
    let mut llr = vec![];
    let mut z = vec![];
    if c.modulation == "OFDM" {
        let n = c.fft_size;
        let cp = n / 8;
        if !f.samples.len().is_multiple_of(n + cp) {
            return Err("OFDM : symbole reçu partiel".into());
        }
        for block in f.samples.chunks(n + cp) {
            let mut carriers = block[cp..].to_vec();
            fft(&mut carriers, false)?;
            z.extend(carriers.into_iter().map(|z| z / (n as f64).sqrt()));
        }
    } else {
        if !f.samples.len().is_multiple_of(c.sps) {
            return Err("Modem : symbole reçu partiel".into());
        }
        if c.modulation == "FSK" {
            if c.order > 16 {
                return Err("FSK ordre >16".into());
            }
            for symbol in f.samples.chunks(c.sps) {
                let score: Vec<_> = (0..c.order)
                    .map(|m| {
                        let hz = (2. * m as f64 - (c.order - 1) as f64) * c.tone_hz;
                        symbol
                            .iter()
                            .enumerate()
                            .fold(Complex::default(), |a, (i, z)| {
                                a + *z
                                    * Complex::polar(1., -2. * PI * hz * i as f64 / f.sample_rate)
                            })
                            .norm2()
                    })
                    .collect();
                let best = score
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .unwrap()
                    .0;
                for bit in (0..k).rev() {
                    let b = ((best >> bit) & 1) as u8;
                    bits.push(b);
                    llr.push(if b == 0 { 8. } else { -8. })
                }
            }
            return Ok(BitFrame {
                bits,
                llr,
                simulated: f.simulated,
            });
        }
        z = f
            .samples
            .chunks(c.sps)
            .map(|s| s.iter().copied().fold(Complex::default(), |a, b| a + b) / s.len() as f64)
            .collect();
    }
    let constellation = points(c)?;
    let variance = 10f64.powf(-c.noise_db / 10.).max(1e-12);
    for z in z {
        let distances: Vec<_> = constellation
            .iter()
            .map(|p| (z - *p * c.amplitude).norm2())
            .collect();
        let best = distances
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        for bit in (0..k).rev() {
            bits.push(((best >> bit) & 1) as u8);
            let min = |value: usize| {
                distances
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| ((i >> bit) & 1) == value)
                    .map(|(_, d)| *d)
                    .fold(f64::INFINITY, f64::min)
            };
            llr.push(((min(1) - min(0)) / variance).clamp(-100., 100.));
        }
    }
    let result = BitFrame {
        bits,
        llr,
        simulated: f.simulated,
    };
    result.validate()?;
    Ok(result)
}
