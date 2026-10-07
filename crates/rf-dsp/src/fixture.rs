//! Local two-port fixture removal. NZC-style time gating requires broadband
//! harmonic sampling; narrow band data is never extrapolated to DC silently.
use rf_core::{dsp::Complex as C, network::TwoPort};
const ONE: C = C { re: 1., im: 0. };
pub fn parse(text: &str) -> Result<TwoPort, String> {
    if text.len() > 2_000_000 {
        return Err("Fichier s2p trop volumineux".into());
    }
    let (mut scale, mut format, mut z0) = (1e9, "MA".to_string(), 50.);
    let mut values = Vec::new();
    let mut header = false;
    for line in text.lines() {
        let line = line.split('!').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            return Err(
                "Touchstone 2.x non pris en charge ; exporter en Touchstone 1.x s2p".into(),
            );
        }
        if let Some(option) = line.strip_prefix('#') {
            if header || !values.is_empty() {
                return Err("En-tête Touchstone multiple/tardif".into());
            }
            header = true;
            let t: Vec<_> = option
                .split_whitespace()
                .map(str::to_ascii_uppercase)
                .collect();
            if t.len() != 5 || t[1] != "S" || t[3] != "R" {
                return Err("Attendu : # Hz S RI R 50 (RI, MA ou DB)".into());
            }
            scale = match t[0].as_str() {
                "HZ" => 1.,
                "KHZ" => 1e3,
                "MHZ" => 1e6,
                "GHZ" => 1e9,
                _ => return Err("Unité Touchstone inconnue".into()),
            };
            format = t[2].clone();
            if !matches!(format.as_str(), "RI" | "MA" | "DB") {
                return Err("Format Touchstone non pris en charge".into());
            }
            z0 = t[4].parse::<f64>().map_err(|e| e.to_string())?;
            continue;
        }
        for v in line.split_whitespace() {
            values.push(
                v.replace(['D', 'd'], "E")
                    .parse::<f64>()
                    .map_err(|e| e.to_string())?,
            );
        }
    }
    if values.len() % 9 != 0 {
        return Err("s2p incomplet : 9 valeurs par fréquence".into());
    }
    let mut data = TwoPort {
        frequency_hz: Vec::new(),
        s: Vec::new(),
        z0,
        simulated: false,
    };
    for row in values.chunks_exact(9) {
        data.frequency_hz.push(row[0] * scale);
        let mut s = [C::default(); 4];
        for k in 0..4 {
            let (a, b) = (row[1 + 2 * k], row[2 + 2 * k]);
            s[k] = match format.as_str() {
                "RI" => C::new(a, b),
                "DB" => C::polar(10f64.powf(a / 20.), b.to_radians()),
                _ => {
                    if a < 0. {
                        return Err("Magnitude MA négative".into());
                    }
                    C::polar(a, b.to_radians())
                }
            };
        }
        data.s.push(s);
    }
    data.validate()?;
    Ok(data)
}
pub fn touchstone(d: &TwoPort) -> Result<String, String> {
    d.validate()?;
    let mut s = format!(
        "! RF Workbench · simulated={}\n# Hz S RI R {}\n",
        d.simulated, d.z0
    );
    for (f, row) in d.frequency_hz.iter().zip(&d.s) {
        s.push_str(&format!("{f:.15e}"));
        for z in row {
            s.push_str(&format!(" {:.15e} {:.15e}", z.re, z.im));
        }
        s.push('\n');
    }
    Ok(s)
}
type M = [[C; 2]; 2];
fn mul(a: M, b: M) -> M {
    std::array::from_fn(|i| std::array::from_fn(|j| a[i][0] * b[0][j] + a[i][1] * b[1][j]))
}
fn inv(a: M) -> Result<M, String> {
    let d = a[0][0] * a[1][1] - a[0][1] * a[1][0];
    if d.norm2() < 1e-24 {
        return Err("Réseau singulier / transmission insuffisante".into());
    }
    Ok([[a[1][1] / d, -a[0][1] / d], [-a[1][0] / d, a[0][0] / d]])
}
fn abcd(s: [C; 4], z: f64) -> Result<M, String> {
    let [a, c, b, d] = s;
    if c.norm2() < 1e-20 {
        return Err("S21 nul / fixture non inversible".into());
    }
    let den = c * 2.;
    Ok([
        [
            ((ONE + a) * (ONE - d) + b * c) / den,
            ((ONE + a) * (ONE + d) - b * c) * z / den,
        ],
        [
            ((ONE - a) * (ONE - d) - b * c) / z / den,
            ((ONE - a) * (ONE + d) + b * c) / den,
        ],
    ])
}
fn s_from(a: M, z: f64) -> Result<[C; 4], String> {
    let [[aa, b], [c, d]] = a;
    let den = aa + b / z + c * z + d;
    if den.norm2() < 1e-24 {
        return Err("Conversion ABCD/S singulière".into());
    }
    Ok([
        (aa + b / z - c * z - d) / den,
        ONE * 2. / den,
        (aa * d - b * c) * 2. / den,
        (-aa + b / z - c * z + d) / den,
    ])
}
pub fn cascade(a: &TwoPort, b: &TwoPort) -> Result<TwoPort, String> {
    compatible(a, b)?;
    let mut d = a.clone();
    d.simulated = a.simulated && b.simulated;
    for (i, row) in d.s.iter_mut().enumerate() {
        *row = s_from(mul(abcd(a.s[i], a.z0)?, abcd(b.s[i], b.z0)?), a.z0)?;
    }
    d.validate()?;
    Ok(d)
}
fn compatible(a: &TwoPort, b: &TwoPort) -> Result<(), String> {
    a.validate()?;
    b.validate()?;
    if (a.z0 - b.z0).abs() > 1e-6
        || a.frequency_hz.len() != b.frequency_hz.len()
        || a.frequency_hz
            .iter()
            .zip(&b.frequency_hz)
            .any(|(a, b)| (a - b).abs() > a.abs().max(1.) * 1e-9)
    {
        return Err("Grilles fréquentielles ou impédances différentes ; aucune interpolation/extrapolation implicite".into());
    }
    Ok(())
}
pub fn deembed(
    raw: &TwoPort,
    left: &TwoPort,
    right: &TwoPort,
    reverse_left: bool,
    reverse_right: bool,
) -> Result<TwoPort, String> {
    compatible(raw, left)?;
    compatible(raw, right)?;
    let (mut a, mut b) = (left.clone(), right.clone());
    if reverse_left {
        a.reverse();
    }
    if reverse_right {
        b.reverse();
    }
    let mut out = raw.clone();
    for (i, row) in out.s.iter_mut().enumerate() {
        *row = s_from(
            mul(
                mul(inv(abcd(a.s[i], raw.z0)?)?, abcd(raw.s[i], raw.z0)?),
                inv(abcd(b.s[i], raw.z0)?)?,
            ),
            raw.z0,
        )?;
    }
    out.validate()?;
    Ok(out)
}
/// Bluestein permits arbitrary point counts using the existing radix-2 FFT.
fn transform(x: &[C], inverse: bool) -> Result<Vec<C>, String> {
    let n = x.len();
    if n == 0 || n > 16384 {
        return Err("IFFT : 1–16384 points".into());
    }
    let m = (2 * n - 1).next_power_of_two();
    let sign = if inverse { 1. } else { -1. };
    let mut a = vec![C::default(); m];
    let mut b = a.clone();
    for i in 0..n {
        let angle = sign * std::f64::consts::PI * (i * i) as f64 / n as f64;
        let w = C::polar(1., angle);
        a[i] = x[i] * w;
        b[i] = w.conj();
        if i > 0 {
            b[m - i] = w.conj();
        }
    }
    crate::fft(&mut a, false)?;
    crate::fft(&mut b, false)?;
    for (i, v) in a.iter_mut().enumerate() {
        *v = *v * b[i];
    }
    crate::fft(&mut a, true)?;
    Ok((0..n)
        .map(|i| {
            a[i] * C::polar(1., sign * std::f64::consts::PI * (i * i) as f64 / n as f64)
                / if inverse { n as f64 } else { 1. }
        })
        .collect())
}
fn impulse(s: &[C], dc: f64) -> Result<Vec<C>, String> {
    let n = s.len();
    let mut x = vec![C::default(); 2 * n];
    x[0] = C::new(dc, 0.);
    for (i, v) in s.iter().enumerate() {
        x[i + 1] = *v;
        if i + 1 < n {
            x[2 * n - i - 1] = v.conj();
        }
    }
    x[n].im = 0.;
    let mut t = transform(&x, true)?;
    t.rotate_left(n);
    Ok(t)
}
fn reflective_dc(s: &[C], f: &[f64]) -> Result<f64, String> {
    // The filtered pre-response step depends linearly on the unknown real DC.
    let n = s.len();
    let df = f[0];
    let filter: Vec<_> = s
        .iter()
        .zip(f)
        .map(|(s, freq)| {
            let q = C::new(0., *freq / (f[n - 1] / 2.));
            *s / (ONE + q * 2.613126 + q * q * 3.414214 + q * q * q * 2.613126 + q * q * q * q)
        })
        .collect();
    let index = (0..2 * n)
        .min_by(|a, b| {
            let time = |i: usize| -1. / df + 2. / df * i as f64 / (2 * n) as f64;
            (time(*a) + 3e-9).abs().total_cmp(&(time(*b) + 3e-9).abs())
        })
        .unwrap();
    let step = |dc| -> Result<f64, String> {
        Ok(impulse(&filter, dc)?
            .iter()
            .take(index + 1)
            .map(|z| z.re)
            .sum())
    };
    let a = step(0.)?;
    let slope = step(1.)? - a;
    if slope.abs() < 1e-12 {
        return Err("Estimation DC mal conditionnée".into());
    }
    Ok(-a / slope)
}
/// Symmetric natural cubic interpolation at DC, using the first nine bins.
fn symmetric_dc(s: &[C]) -> f64 {
    let x: Vec<f64> = (-9..=-1).chain(1..=9).map(f64::from).collect();
    let y: Vec<_> = (0..9).rev().chain(0..9).map(|i| s[i].re).collect();
    let n = x.len();
    let mut diagonal = vec![1.; n];
    let mut upper = vec![0.; n];
    let mut rhs = vec![0.; n];
    for i in 1..n - 1 {
        let a = x[i] - x[i - 1];
        let b = x[i + 1] - x[i];
        diagonal[i] = 2. * (a + b);
        upper[i] = b;
        rhs[i] = 6. * ((y[i + 1] - y[i]) / b - (y[i] - y[i - 1]) / a);
        let factor = a / diagonal[i - 1];
        diagonal[i] -= factor * upper[i - 1];
        rhs[i] -= factor * rhs[i - 1];
    }
    let mut second = vec![0.; n];
    for i in (0..n - 1).rev() {
        second[i] = (rhs[i] - upper[i] * second[i + 1]) / diagonal[i];
    }
    (y[8] + y[9]) / 2. - (second[8] + second[9]) / 4.
}
fn gate(s: &[C], f: &[f64], cut: usize) -> Result<Vec<C>, String> {
    let mut t = impulse(s, reflective_dc(s, f)?)?;
    for v in &mut t[cut..] {
        *v = C::default();
    }
    let n = s.len();
    t.rotate_left(n);
    let x = transform(&t, false)?;
    Ok(x[1..=n].to_vec())
}
fn continuous_roots(v: Vec<C>) -> Vec<C> {
    let mut out = Vec::<C>::new();
    for z in v {
        let mut root = C::polar(z.norm2().sqrt().sqrt(), z.arg() / 2.);
        if out
            .last()
            .is_some_and(|p| (-root - *p).norm2() < (root - *p).norm2())
        {
            root = -root;
        }
        out.push(root);
    }
    out
}
fn renormalize(s: [C; 4], old: f64, new: f64) -> Result<[C; 4], String> {
    s_from(abcd(s, old)?, new)
}
#[derive(Clone, Debug)]
pub struct Extraction {
    pub left: TwoPort,
    pub right: TwoPort,
    pub time_s: Vec<f64>,
    pub transmission_impulse: Vec<f64>,
    pub impedance_ohm: Vec<f64>,
    pub split_time_s: f64,
    pub split_impedance_ohm: f64,
    pub residual_db: f64,
    pub residual_deg: f64,
    pub warnings: Vec<String>,
}
/// Equal electrical half lengths and a reciprocal passive 2xThru are assumed.
/// Right fixture is stored connector->DUT; reverse it in the output cascade.
pub fn split(thru: &TwoPort, forced_z: Option<f64>) -> Result<Extraction, String> {
    thru.validate()?;
    let n = thru.s.len();
    let f = &thru.frequency_hz;
    if !(16..=8192).contains(&n)
        || f[0] <= 0.
        || f.iter()
            .enumerate()
            .any(|(i, v)| (*v - f[0] * (i + 1) as f64).abs() > *v * 1e-7)
    {
        return Err("2×Thru temporel : mesurer de Δf à N×Δf (16–8192 points, pas uniforme, bande proche de DC). Un fichier 36–38 GHz seul ne permet pas cette extraction sans extrapolation non maîtrisée.".into());
    }
    if thru
        .s
        .iter()
        .any(|s| (s[1] - s[2]).norm2() > 1e-4 || s.iter().any(|z| z.norm2() > 1.001))
    {
        return Err("2×Thru doit être passif et réciproque (tolérance S21/S12 = 0.01)".into());
    }
    let col = |k: usize| thru.s.iter().map(|s| s[k]).collect::<Vec<_>>();
    let s21 = col(1);
    let dc = symmetric_dc(&s21);
    let t = impulse(&s21, dc)?;
    let cut = (n..2 * n)
        .max_by(|a, b| t[*a].re.total_cmp(&t[*b].re))
        .unwrap();
    let delay = (cut - n) as f64 / (2 * n) as f64 / f[0];
    if cut < n + 3 || cut > 2 * n - 4 {
        return Err("2×Thru trop court pour séparer les launches, ou délai replié ; élargir la bande / réduire Δf".into());
    }
    let reflection = impulse(&col(0), reflective_dc(&col(0), f)?)?;
    let mut accum = 0.;
    let z: Vec<_> = reflection
        .iter()
        .map(|v| {
            accum += v.re;
            thru.z0 * (1. + accum) / (1. - accum)
        })
        .collect();
    let zx = forced_z.unwrap_or((z[cut - 1] + z[cut]) / 2.);
    if !zx.is_finite() || !(1. ..=1000.).contains(&zx) {
        return Err("Impédance au plan de coupe invalide".into());
    }
    let mut normal = thru.clone();
    for s in &mut normal.s {
        *s = renormalize(*s, thru.z0, zx)?;
    }
    let c = |k: usize| normal.s.iter().map(|s| s[k]).collect::<Vec<_>>();
    let (a, d) = (gate(&c(0), f, cut)?, gate(&c(3), f, cut)?);
    let mut r1 = Vec::new();
    let mut r2 = Vec::new();
    for (i, s) in normal.s.iter().enumerate() {
        if s[1].norm2() < 1e-20 || s[2].norm2() < 1e-20 {
            return Err("Transmission 2×Thru insuffisante".into());
        }
        r1.push((s[3] - d[i]) / s[2]);
        r2.push((s[0] - a[i]) / s[1]);
    }
    let h1 = continuous_roots(
        normal
            .s
            .iter()
            .enumerate()
            .map(|(i, s)| s[1] * (ONE - r1[i] * r2[i]))
            .collect(),
    );
    let h2 = continuous_roots(
        normal
            .s
            .iter()
            .enumerate()
            .map(|(i, s)| s[2] * (ONE - r1[i] * r2[i]))
            .collect(),
    );
    let (mut left, mut right) = (thru.clone(), thru.clone());
    for i in 0..n {
        left.s[i] = renormalize([a[i], h1[i], h1[i], r1[i]], zx, thru.z0)?;
        right.s[i] = renormalize([d[i], h2[i], h2[i], r2[i]], zx, thru.z0)?;
    }
    left.validate()?;
    right.validate()?;
    let residual = deembed(thru, &left, &right, false, true)?;
    let residual_db = residual
        .s
        .iter()
        .map(|s| (10. * s[1].norm2().log10()).abs())
        .fold(0., f64::max);
    let residual_deg = residual
        .s
        .iter()
        .map(|s| s[1].arg().to_degrees().abs())
        .fold(0., f64::max);
    let mut warnings=vec!["Méthode NZC temporelle : demi-fixtures de longueurs électriques égales ; impédance du 2×Thru représentative du montage. Ce rapport n'est pas une certification IEEE 370.".into()];
    if residual_db > 0.1 || residual_deg > 1. {
        warnings.push("Autodéembedding hors seuil ±0.1 dB / ±1° : contrôler fixture, grille, DC et plan de coupe.".into());
    }
    Ok(Extraction {
        left,
        right,
        time_s: (0..2 * n)
            .map(|i| (i as f64 - n as f64) / (2 * n) as f64 / f[0])
            .collect(),
        transmission_impulse: t.iter().map(|z| z.re).collect(),
        impedance_ohm: z,
        split_time_s: delay / 2.,
        split_impedance_ohm: zx,
        residual_db,
        residual_deg,
        warnings,
    })
}
pub fn demo() -> (TwoPort, TwoPort) {
    let f: Vec<_> = (1..=256).map(|i| i as f64 * 100e6).collect();
    let line = |loss: f64, delay: f64| TwoPort {
        frequency_hz: f.clone(),
        s: f.iter()
            .map(|f| {
                let t = C::polar(10f64.powf(-loss / 20.), -std::f64::consts::TAU * f * delay);
                [C::default(), t, t, C::default()]
            })
            .collect(),
        z0: 50.,
        simulated: true,
    };
    let a = line(1., 0.25e-9);
    let b = line(2., 0.25e-9);
    let dut = line(-20., 0.10e-9);
    (
        cascade(&a, &b).unwrap(),
        cascade(&cascade(&a, &dut).unwrap(), &b).unwrap(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ri_ma_db_roundtrip_and_reverse() {
        let (a, _) = demo();
        let mut d = parse(&touchstone(&a).unwrap()).unwrap();
        for (i, s) in a.s.iter().enumerate() {
            assert!((s[1] - d.s[i][1]).norm2() < 1e-25);
        }
        d.reverse();
        d.reverse();
        assert_eq!(d.s, parse(&touchstone(&a).unwrap()).unwrap().s);
        assert!(parse("# GHz S RI R 50\n1 0 0").is_err());
        let d =
            parse("# MHz S DB R 50\n1 -20 0 -3 90 -3 90 -20 0\n2 -20 0 -3 90 -3 90 -20 0").unwrap();
        assert!((d.s[0][1].arg().to_degrees() - 90.).abs() < 1e-8);
    }
    #[test]
    fn extraction_self_residual_and_known_gain() {
        let (thru, raw) = demo();
        let e = split(&thru, Some(50.)).unwrap();
        assert!(e.residual_db < 1e-7 && e.residual_deg < 1e-7);
        let dut = deembed(&raw, &e.left, &e.right, false, true).unwrap();
        for s in dut.s {
            assert!((10. * s[1].norm2().log10() - 20.).abs() < 1e-7);
        }
    }
    #[test]
    fn refuse_narrowband_zero_transmission_and_mismatched_grid() {
        let (mut thru, raw) = demo();
        thru.frequency_hz.iter_mut().for_each(|f| *f += 36e9);
        assert!(split(&thru, None).is_err());
        let (mut thru, _) = demo();
        thru.s[12][1] = C::default();
        assert!(split(&thru, Some(50.)).is_err());
        assert!(deembed(&raw, &thru, &raw, false, false).is_err());
    }
}
