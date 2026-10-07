//! Reference FEC profiles: convolutional K=3, RS GF(256), sparse LDPC,
//! and parallel RSC turbo. These are not DVB/3GPP standard profiles.
#![allow(clippy::needless_range_loop)] // Trellis and finite-field matrix coordinates.
use crate::Result;
use rf_core::dsp::*;
pub fn process(op: Op, f: &BitFrame, c: &Settings) -> Result<BitFrame> {
    c.validate()?;
    f.validate()?;
    let soft: Vec<_> = if f.llr.is_empty() {
        f.bits
            .iter()
            .map(|b| if *b == 0 { 8. } else { -8. })
            .collect()
    } else {
        f.llr.clone()
    };
    let bits = match op {
        Op::ConvEncode => conv_encode(&f.bits),
        Op::ConvDecode => viterbi(&soft)?,
        Op::RsEncode | Op::RsDecode => {
            if !f.bits.len().is_multiple_of(8) {
                return Err("RS : octets complets requis".into());
            }
            let bytes: Vec<_> = f
                .bits
                .chunks(8)
                .map(|b| b.iter().fold(0u8, |a, b| (a << 1) | b))
                .collect();
            let parity = c.rs_parity;
            let size = if op == Op::RsEncode {
                255 - parity
            } else {
                255
            };
            if bytes.len() % size != 0 {
                return Err(format!("RS : blocs de {size} octets requis"));
            }
            let mut output = vec![];
            for block in bytes.chunks(size) {
                let mut block = block.to_vec();
                if op == Op::RsEncode {
                    block = rs_encode(&block, parity)?
                } else {
                    rs_decode(&mut block, parity)?;
                    block.truncate(255 - parity);
                }
                for byte in block {
                    output.extend((0..8).rev().map(|b| (byte >> b) & 1));
                }
            }
            output
        }
        Op::LdpcEncode => ldpc_encode(&f.bits)?,
        Op::LdpcDecode => ldpc_decode(&soft, c.iterations)?,
        Op::TurboEncode => turbo_encode(&f.bits)?,
        Op::TurboDecode => turbo_decode(&soft, c.iterations)?,
        _ => return Err("Code canal inconnu".into()),
    };
    let result = BitFrame {
        bits,
        llr: vec![],
        simulated: f.simulated,
    };
    result.validate()?;
    Ok(result)
}
pub fn conv_encode(bits: &[u8]) -> Vec<u8> {
    let mut state = 0;
    let mut out = vec![];
    for b in bits.iter().copied().chain([0, 0]) {
        let register = (state << 1) | b as usize;
        out.extend([
            (register & 7).count_ones() as u8 & 1,
            (register & 5).count_ones() as u8 & 1,
        ]);
        state = register & 3;
    }
    out
}
pub fn viterbi(llr: &[f64]) -> Result<Vec<u8>> {
    if !llr.len().is_multiple_of(2) || llr.len() < 6 {
        return Err("Viterbi : trame terminée K=3, rate 1/2 requise".into());
    }
    let count = llr.len() / 2;
    let mut metrics = [f64::INFINITY; 4];
    metrics[0] = 0.;
    let mut history = vec![[0usize; 4]; count];
    for t in 0..count {
        let mut next = [f64::INFINITY; 4];
        for state in 0usize..4 {
            for bit in 0..2 {
                let reg = (state << 1) | bit;
                let dest = reg & 3;
                let a = (reg & 7).count_ones() as usize & 1;
                let b = (reg & 5).count_ones() as usize & 1;
                let metric = metrics[state]
                    - (1. - 2. * a as f64) * llr[2 * t]
                    - (1. - 2. * b as f64) * llr[2 * t + 1];
                if metric < next[dest] {
                    next[dest] = metric;
                    history[t][dest] = state;
                }
            }
        }
        metrics = next;
    }
    let mut state = 0;
    let mut out = vec![0; count];
    for t in (0..count).rev() {
        out[t] = (state & 1) as u8;
        state = history[t][state]
    }
    out.truncate(count - 2);
    Ok(out)
}
fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut r = 0;
    while b > 0 {
        if b & 1 != 0 {
            r ^= a
        }
        let carry = a & 0x80;
        a <<= 1;
        if carry != 0 {
            a ^= 0x1d
        }
        b >>= 1;
    }
    r
}
fn gf_pow(a: u8, n: usize) -> u8 {
    (0..n).fold(1, |v, _| gf_mul(v, a))
}
fn gf_inv(a: u8) -> u8 {
    gf_pow(a, 254)
}
fn evaluate(poly: &[u8], x: u8) -> u8 {
    poly.iter().fold(0, |v, c| gf_mul(v, x) ^ c)
}
pub fn rs_encode(data: &[u8], parity: usize) -> Result<Vec<u8>> {
    if data.is_empty() || data.len() + parity > 255 || parity < 2 {
        return Err("Bloc RS invalide".into());
    }
    let mut generator = vec![1];
    for i in 0..parity {
        let mut next = vec![0; generator.len() + 1];
        for (j, c) in generator.iter().enumerate() {
            next[j] ^= *c;
            next[j + 1] ^= gf_mul(*c, gf_pow(2, i));
        }
        generator = next;
    }
    let mut out = data.to_vec();
    out.resize(data.len() + parity, 0);
    for i in 0..data.len() {
        let factor = out[i];
        for j in 1..generator.len() {
            out[i + j] ^= gf_mul(generator[j], factor)
        }
    }
    out[..data.len()].copy_from_slice(data);
    Ok(out)
}
pub fn rs_decode(code: &mut [u8], parity: usize) -> Result<usize> {
    if code.len() > 255 || code.len() <= parity {
        return Err("Bloc RS invalide".into());
    }
    let syndrome: Vec<_> = (0..parity).map(|i| evaluate(code, gf_pow(2, i))).collect();
    if syndrome.iter().all(|s| *s == 0) {
        return Ok(0);
    }
    let mut c = vec![0; parity + 1];
    let mut b = c.clone();
    c[0] = 1;
    b[0] = 1;
    let (mut length, mut shift, mut prev) = (0usize, 1usize, 1u8);
    for n in 0..parity {
        let mut discrepancy = syndrome[n];
        for i in 1..=length {
            discrepancy ^= gf_mul(c[i], syndrome[n - i]);
        }
        if discrepancy == 0 {
            shift += 1;
            continue;
        }
        let old = c.clone();
        let scale = gf_mul(discrepancy, gf_inv(prev));
        for j in 0..=parity - shift {
            c[j + shift] ^= gf_mul(scale, b[j]);
        }
        if 2 * length <= n {
            length = n + 1 - length;
            b = old;
            prev = discrepancy;
            shift = 1
        } else {
            shift += 1
        }
    }
    if length == 0 || length * 2 > parity {
        return Err("RS : capacité de correction dépassée".into());
    }
    let mut positions = vec![];
    let mut x = vec![];
    for pos in 0..code.len() {
        let locator = gf_pow(2, code.len() - 1 - pos);
        let inverse = gf_inv(locator);
        let mut v = 0;
        let mut power = 1;
        for coeff in &c[..=length] {
            v ^= gf_mul(*coeff, power);
            power = gf_mul(power, inverse);
        }
        if v == 0 {
            positions.push(pos);
            x.push(locator)
        }
    }
    if positions.len() != length {
        return Err("RS : localisateur non résolu".into());
    }
    let mut matrix = vec![vec![0; length + 1]; length];
    for row in 0..length {
        for col in 0..length {
            matrix[row][col] = gf_pow(x[col], row)
        }
        matrix[row][length] = syndrome[row];
    }
    for col in 0..length {
        let pivot = (col..length)
            .find(|r| matrix[*r][col] != 0)
            .ok_or("RS : matrice singulière")?;
        matrix.swap(col, pivot);
        let inv = gf_inv(matrix[col][col]);
        for j in col..=length {
            matrix[col][j] = gf_mul(matrix[col][j], inv)
        }
        for row in 0..length {
            if row == col {
                continue;
            }
            let scale = matrix[row][col];
            for j in col..=length {
                matrix[row][j] ^= gf_mul(scale, matrix[col][j]);
            }
        }
    }
    let mut corrected = code.to_vec();
    for i in 0..length {
        corrected[positions[i]] ^= matrix[i][length]
    }
    if (0..parity).any(|i| evaluate(&corrected, gf_pow(2, i)) != 0) {
        return Err("RS : syndromes résiduels".into());
    }
    code.copy_from_slice(&corrected);
    Ok(length)
}
// Sparse systematic (128,64) H=[A|I], three data neighbours/check.
fn checks() -> Vec<Vec<usize>> {
    (0..64)
        .map(|r| vec![r, (r + 7) % 64, (r + 23) % 64, 64 + r])
        .collect()
}
pub fn ldpc_encode(bits: &[u8]) -> Result<Vec<u8>> {
    if !bits.len().is_multiple_of(64) {
        return Err("LDPC : blocs de 64 bits".into());
    }
    let mut out = vec![];
    for b in bits.chunks(64) {
        out.extend_from_slice(b);
        out.extend((0..64).map(|r| b[r] ^ b[(r + 7) % 64] ^ b[(r + 23) % 64]));
    }
    Ok(out)
}
pub fn ldpc_decode(llr: &[f64], iterations: usize) -> Result<Vec<u8>> {
    if !llr.len().is_multiple_of(128) {
        return Err("LDPC : blocs de 128 LLR".into());
    }
    let h = checks();
    let mut output = vec![];
    for block in llr.chunks(128) {
        let mut q: Vec<Vec<f64>> = h
            .iter()
            .map(|row| row.iter().map(|j| block[*j]).collect())
            .collect();
        let mut r = vec![vec![0.; 4]; 64];
        let mut hard = [0; 128];
        let mut valid = false;
        for _ in 0..iterations {
            for i in 0..64 {
                for j in 0..4 {
                    let mut sign = 1.;
                    let mut min = f64::INFINITY;
                    for k in 0..4 {
                        if k == j {
                            continue;
                        }
                        sign *= q[i][k].signum();
                        min = min.min(q[i][k].abs());
                    }
                    r[i][j] = 0.8 * sign * min;
                }
            }
            let mut posterior = block.to_vec();
            for (i, row) in h.iter().enumerate() {
                for (j, col) in row.iter().enumerate() {
                    posterior[*col] += r[i][j];
                }
            }
            for j in 0..128 {
                hard[j] = u8::from(posterior[j] < 0.);
            }
            if h.iter()
                .all(|row| row.iter().fold(0, |v, j| v ^ hard[*j]) == 0)
            {
                valid = true;
                break;
            }
            for (i, row) in h.iter().enumerate() {
                for (j, col) in row.iter().enumerate() {
                    q[i][j] = posterior[*col] - r[i][j];
                }
            }
        }
        if !valid {
            return Err("LDPC : syndrome non nul après itérations".into());
        }
        output.extend_from_slice(&hard[..64]);
    }
    Ok(output)
}
fn transition(state: usize, bit: usize) -> (usize, usize) {
    let feedback = bit ^ ((state >> 1) & 1) ^ (state & 1);
    let parity = feedback ^ ((state >> 1) & 1);
    (((state << 1) | feedback) & 3, parity)
}
fn permutation(n: usize) -> Vec<usize> {
    // reversal is an explicit bijective interleaver.
    (0..n).map(|i| n - 1 - i).collect()
}
pub fn turbo_encode(bits: &[u8]) -> Result<Vec<u8>> {
    if bits.len() > 8192 {
        return Err("Turbo : ≤8192 bits".into());
    }
    let p = permutation(bits.len());
    let (mut a, mut b) = (0, 0);
    let mut out = vec![];
    for i in 0..bits.len() {
        let (na, pa) = transition(a, bits[i] as usize);
        let (nb, pb) = transition(b, bits[p[i]] as usize);
        out.extend([bits[i], pa as u8, pb as u8]);
        a = na;
        b = nb;
    }
    Ok(out)
}
fn bcjr(systematic: &[f64], parity: &[f64], prior: &[f64]) -> Vec<f64> {
    let n = systematic.len();
    let mut alpha = vec![[-1e30_f64; 4]; n + 1];
    let mut beta = vec![[0.0_f64; 4]; n + 1];
    alpha[0][0] = 0.;
    let metric = |i: usize, b: usize, p: usize| {
        0.5 * ((1. - 2. * b as f64) * (systematic[i] + prior[i]) + (1. - 2. * p as f64) * parity[i])
    };
    for i in 0..n {
        for s in 0..4 {
            for b in 0..2 {
                let (d, p) = transition(s, b);
                alpha[i + 1][d] = alpha[i + 1][d].max(alpha[i][s] + metric(i, b, p));
            }
        }
        let max = alpha[i + 1]
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        for x in &mut alpha[i + 1] {
            *x -= max;
        }
    }
    for i in (0..n).rev() {
        beta[i] = [-1e30; 4];
        for s in 0..4 {
            for b in 0..2 {
                let (d, p) = transition(s, b);
                beta[i][s] = beta[i][s].max(beta[i + 1][d] + metric(i, b, p));
            }
        }
        let max = beta[i].iter().copied().fold(f64::NEG_INFINITY, f64::max);
        for x in &mut beta[i] {
            *x -= max;
        }
    }
    let mut output = vec![0.; n];
    for i in 0..n {
        let mut max = [-1e30f64; 2];
        for s in 0..4 {
            for b in 0..2 {
                let (d, p) = transition(s, b);
                max[b] = max[b].max(alpha[i][s] + metric(i, b, p) + beta[i + 1][d]);
            }
        }
        output[i] = (max[0] - max[1] - systematic[i] - prior[i]).clamp(-100., 100.);
    }
    output
}
pub fn turbo_decode(llr: &[f64], iterations: usize) -> Result<Vec<u8>> {
    if !llr.len().is_multiple_of(3) || llr.len() > 24576 {
        return Err("Turbo : rate 1/3, ≤8192 bits utiles".into());
    }
    let n = llr.len() / 3;
    let p = permutation(n);
    let sys: Vec<_> = llr.chunks(3).map(|c| c[0]).collect();
    let pa: Vec<_> = llr.chunks(3).map(|c| c[1]).collect();
    let pb: Vec<_> = llr.chunks(3).map(|c| c[2]).collect();
    let inter: Vec<_> = p.iter().map(|i| sys[*i]).collect();
    let mut prior = vec![0.; n];
    let mut posterior = vec![0.; n];
    for _ in 0..iterations {
        let a = bcjr(&sys, &pa, &prior);
        let a_inter: Vec<_> = p.iter().map(|i| a[*i]).collect();
        let b = bcjr(&inter, &pb, &a_inter);
        for i in 0..n {
            prior[p[i]] = b[i];
            posterior[p[i]] = inter[i] + a_inter[i] + b[i];
        }
    }
    Ok(posterior.iter().map(|v| u8::from(*v < 0.)).collect())
}
