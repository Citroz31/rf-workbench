//! Typed baseband contracts. Spectral density is distinct from RF power.
use serde::{Deserialize, Serialize};
use std::ops::{Add, Div, Mul, Neg, Sub};
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}
impl Complex {
    pub fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }
    pub fn polar(r: f64, p: f64) -> Self {
        Self::new(r * p.cos(), r * p.sin())
    }
    pub fn norm2(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
    pub fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }
    pub fn conj(self) -> Self {
        Self::new(self.re, -self.im)
    }
}
impl Add for Complex {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self::new(self.re + b.re, self.im + b.im)
    }
}
impl Sub for Complex {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        Self::new(self.re - b.re, self.im - b.im)
    }
}
impl Mul for Complex {
    type Output = Self;
    fn mul(self, b: Self) -> Self {
        Self::new(
            self.re * b.re - self.im * b.im,
            self.re * b.im + self.im * b.re,
        )
    }
}
impl Mul<f64> for Complex {
    type Output = Self;
    fn mul(self, b: f64) -> Self {
        Self::new(self.re * b, self.im * b)
    }
}
impl Div<f64> for Complex {
    type Output = Self;
    fn div(self, b: f64) -> Self {
        self * (1. / b)
    }
}
impl Neg for Complex {
    type Output = Self;
    fn neg(self) -> Self {
        self * (-1.)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    Hz,
    Dbm,
    Dbv,
    Dbfs,
    Db,
    Volt,
    Ampere,
    Second,
    SamplesPerSecond,
    Watt,
    Fs,
    Percent,
    Ratio,
    DbVolt2PerHz,
    DbFs2PerHz,
    DbcPerHz,
}
impl Unit {
    pub const ALL: [Self; 16] = [
        Self::Hz,
        Self::Dbm,
        Self::Dbv,
        Self::Dbfs,
        Self::Db,
        Self::Volt,
        Self::Ampere,
        Self::Second,
        Self::SamplesPerSecond,
        Self::Watt,
        Self::Fs,
        Self::Percent,
        Self::Ratio,
        Self::DbVolt2PerHz,
        Self::DbFs2PerHz,
        Self::DbcPerHz,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Hz => "Hz",
            Self::Dbm => "dBm",
            Self::Dbv => "dBV",
            Self::Dbfs => "dBFS",
            Self::Db => "dB",
            Self::Volt => "V",
            Self::Ampere => "A",
            Self::Second => "s",
            Self::SamplesPerSecond => "sample/s",
            Self::Watt => "W",
            Self::Fs => "FS",
            Self::Percent => "%",
            Self::Ratio => "ratio",
            Self::DbVolt2PerHz => "dBV²/Hz",
            Self::DbFs2PerHz => "dBFS²/Hz",
            Self::DbcPerHz => "dBc/Hz",
        }
    }
}
pub fn convert(value: f64, from: Unit, to: Unit, impedance: f64) -> Result<f64, String> {
    if !value.is_finite() || !impedance.is_finite() || impedance <= 0. {
        return Err("Valeur/impédance invalide".into());
    }
    if from == to {
        return Ok(value);
    }
    if matches!(from, Unit::Volt | Unit::Ampere) && value < 0. {
        return Err("Amplitude RMS négative".into());
    }
    let watts = match from {
        Unit::Watt => Some(value),
        Unit::Dbv => Some(10f64.powf(value / 10.) / impedance),
        Unit::Dbm => Some(0.001 * 10f64.powf(value / 10.)),
        Unit::Volt => Some(value * value / impedance),
        Unit::Ampere => Some(value * value * impedance),
        _ => None,
    };
    let result = if let Some(w) = watts {
        if w < 0. {
            return Err("Puissance négative".into());
        }
        match to {
            Unit::Watt => w,
            Unit::Dbv if w > 0. => 10. * (w * impedance).log10(),
            Unit::Dbm if w > 0. => 10. * (w / 0.001).log10(),
            Unit::Volt => (w * impedance).sqrt(),
            Unit::Ampere => (w / impedance).sqrt(),
            _ => return Err("Conversion dimensionnelle impossible".into()),
        }
    } else {
        match (from, to) {
            (Unit::Ratio, Unit::Db) if value > 0. => 10. * value.log10(),
            (Unit::Db, Unit::Ratio) => 10f64.powf(value / 10.),
            (Unit::Percent, Unit::Ratio) => value / 100.,
            (Unit::Ratio, Unit::Percent) => value * 100.,
            _ => return Err("Conversion dimensionnelle impossible".into()),
        }
    };
    if result.is_finite() {
        Ok(result)
    } else {
        Err("Conversion non finie".into())
    }
}
/// Parse an SI-prefixed quantity into base units. dBm and dB remain logarithmic.
pub fn parse_quantity(text: &str) -> Result<(f64, Unit), String> {
    let text = text.trim();
    let suffixes = [
        ("sample/s", Unit::SamplesPerSecond),
        ("dBm", Unit::Dbm),
        ("dBV", Unit::Dbv),
        ("dBFS", Unit::Dbfs),
        ("dB", Unit::Db),
        ("Hz", Unit::Hz),
        ("V", Unit::Volt),
        ("A", Unit::Ampere),
        ("s", Unit::Second),
        ("W", Unit::Watt),
        ("%", Unit::Percent),
    ];
    for (suffix, unit) in suffixes {
        if let Some(prefix) = text.strip_suffix(suffix) {
            let prefix = prefix.trim();
            let (raw, factor) = if matches!(
                unit,
                Unit::Dbm | Unit::Dbv | Unit::Dbfs | Unit::Db | Unit::Percent
            ) {
                (prefix, 1.)
            } else {
                match prefix.chars().last() {
                    Some('G') => (&prefix[..prefix.len() - 1], 1e9),
                    Some('M') => (&prefix[..prefix.len() - 1], 1e6),
                    Some('k') => (&prefix[..prefix.len() - 1], 1e3),
                    Some('m') => (&prefix[..prefix.len() - 1], 1e-3),
                    Some('u') => (&prefix[..prefix.len() - 1], 1e-6),
                    Some('µ') => (&prefix[..prefix.len() - 'µ'.len_utf8()], 1e-6),
                    Some('n') => (&prefix[..prefix.len() - 1], 1e-9),
                    _ => (prefix, 1.),
                }
            };
            let value = raw.trim().parse::<f64>().map_err(|_| "Quantité invalide")? * factor;
            if !value.is_finite() {
                return Err("Quantité non finie".into());
            }
            return Ok((value, unit));
        }
    }
    Err("Unité inconnue".into())
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimeTag {
    pub first_sample: u64,
    pub epoch_ns: Option<i64>,
    pub clock_domain: String,
    pub discontinuity: bool,
}
impl Default for TimeTag {
    fn default() -> Self {
        Self {
            first_sample: 0,
            epoch_ns: None,
            clock_domain: "simulation".into(),
            discontinuity: false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IqFrame {
    pub samples: Vec<Complex>,
    pub sample_rate: f64,
    pub center_hz: f64,
    pub unit: Unit,
    pub simulated: bool,
    pub time: TimeTag,
}
impl IqFrame {
    pub fn validate(&self) -> Result<(), String> {
        if self
            .time
            .first_sample
            .checked_add(self.samples.len() as u64)
            .is_none()
            || self.samples.is_empty()
            || self.samples.len() > 65536
            || !self.sample_rate.is_finite()
            || self.sample_rate <= 0.
            || !self.center_hz.is_finite()
            || self.center_hz < 0.
            || !matches!(self.unit, Unit::Volt | Unit::Fs)
            || self
                .samples
                .iter()
                .any(|z| !z.re.is_finite() || !z.im.is_finite())
            || self.time.clock_domain.len() > 128
        {
            return Err("Trame I/Q invalide".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BitFrame {
    pub bits: Vec<u8>,
    pub llr: Vec<f64>,
    pub simulated: bool,
}
impl BitFrame {
    pub fn validate(&self) -> Result<(), String> {
        if self.bits.is_empty()
            || self.bits.len() > 524288
            || self.bits.iter().any(|b| *b > 1)
            || (!self.llr.is_empty() && self.llr.len() != self.bits.len())
            || self.llr.iter().any(|x| !x.is_finite())
        {
            return Err("Bits/LLR invalides".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spectrum {
    pub frequency_hz: Vec<f64>,
    pub levels: Vec<f64>,
    pub unit: Unit,
    pub simulated: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Data {
    Iq(IqFrame),
    Bits(BitFrame),
    Spectrum(Spectrum),
    Quantity {
        value: f64,
        unit: Unit,
        simulated: bool,
    },
}
impl Data {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Iq(x) => x.validate(),
            Self::Bits(x) => x.validate(),
            Self::Spectrum(s) => {
                if s.levels.len() < 2
                    || s.levels.len() > 65536
                    || s.levels.len() != s.frequency_hz.len()
                    || s.levels
                        .iter()
                        .chain(&s.frequency_hz)
                        .any(|x| !x.is_finite())
                    || s.frequency_hz.windows(2).any(|w| w[1] <= w[0])
                {
                    Err("Spectre DSP invalide".into())
                } else {
                    Ok(())
                }
            }
            Self::Quantity { value, .. } => {
                if value.is_finite() {
                    Ok(())
                } else {
                    Err("Mesure indéfinie : signal nul ou rapport infini".into())
                }
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op {
    IqSource,
    BitSource,
    IoSource,
    IoSink,
    Fir,
    Iir,
    Decimate,
    Interpolate,
    Fft,
    Window,
    Agc,
    Pll,
    Costas,
    SymbolTiming,
    FrameSync,
    DigitalMod,
    DigitalDemod,
    AnalogMod,
    AnalogDemod,
    ConvEncode,
    ConvDecode,
    RsEncode,
    RsDecode,
    LdpcEncode,
    LdpcDecode,
    TurboEncode,
    TurboDecode,
    Power,
    Snr,
    Thd,
    Evm,
    Ber,
    Per,
    PhaseNoise,
    Vswr,
    Psd,
    Spectrogram,
    SpectralPeak,
    DcOffset,
    IqBalance,
    Cable,
    CalibrationTable,
    Uncertainty,
    Convert,
    Channel,
}
impl Op {
    pub const ALL: [Self; 45] = [
        Self::IqSource,
        Self::BitSource,
        Self::IoSource,
        Self::IoSink,
        Self::Fir,
        Self::Iir,
        Self::Decimate,
        Self::Interpolate,
        Self::Fft,
        Self::Window,
        Self::Agc,
        Self::Pll,
        Self::Costas,
        Self::SymbolTiming,
        Self::FrameSync,
        Self::DigitalMod,
        Self::DigitalDemod,
        Self::AnalogMod,
        Self::AnalogDemod,
        Self::ConvEncode,
        Self::ConvDecode,
        Self::RsEncode,
        Self::RsDecode,
        Self::LdpcEncode,
        Self::LdpcDecode,
        Self::TurboEncode,
        Self::TurboDecode,
        Self::Power,
        Self::Snr,
        Self::Thd,
        Self::Evm,
        Self::Ber,
        Self::Per,
        Self::PhaseNoise,
        Self::Vswr,
        Self::Psd,
        Self::Spectrogram,
        Self::SpectralPeak,
        Self::DcOffset,
        Self::IqBalance,
        Self::Cable,
        Self::CalibrationTable,
        Self::Uncertainty,
        Self::Convert,
        Self::Channel,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::IqSource => "Source I/Q",
            Self::BitSource => "Source bits",
            Self::IoSource => "Entrée HAL / I/Q",
            Self::IoSink => "Sortie HAL / enregistrement",
            Self::Fir => "FIR",
            Self::Iir => "IIR",
            Self::Decimate => "Décimation",
            Self::Interpolate => "Interpolation",
            Self::Fft => "FFT",
            Self::Window => "Fenêtrage",
            Self::Agc => "AGC",
            Self::Pll => "PLL",
            Self::Costas => "Costas",
            Self::SymbolTiming => "Symbol timing",
            Self::FrameSync => "Frame sync",
            Self::DigitalMod => "Modulation ASK/FSK/PSK/QAM/OFDM",
            Self::DigitalDemod => "Démodulation numérique",
            Self::AnalogMod => "Modulation AM/FM/PM",
            Self::AnalogDemod => "Démodulation AM/FM/PM",
            Self::ConvEncode => "Code convolutionnel",
            Self::ConvDecode => "Viterbi",
            Self::RsEncode => "Reed-Solomon encode",
            Self::RsDecode => "Reed-Solomon decode",
            Self::LdpcEncode => "LDPC encode",
            Self::LdpcDecode => "LDPC decode",
            Self::TurboEncode => "Turbo encode",
            Self::TurboDecode => "Turbo decode",
            Self::Power => "Puissance I/Q",
            Self::Snr => "SNR",
            Self::Thd => "THD",
            Self::Evm => "EVM",
            Self::Ber => "BER",
            Self::Per => "PER",
            Self::PhaseNoise => "Phase noise",
            Self::Vswr => "VSWR",
            Self::Psd => "PSD Welch",
            Self::Spectrogram => "Spectrogramme",
            Self::SpectralPeak => "Pic spectral",
            Self::DcOffset => "Offset DC",
            Self::IqBalance => "IQ imbalance",
            Self::Cable => "Compensation câble",
            Self::CalibrationTable => "Table calibration",
            Self::Uncertainty => "Incertitudes",
            Self::Convert => "Conversion unités",
            Self::Channel => "Canal RF",
        }
    }
    pub fn category(self) -> &'static str {
        match self {
            Self::IqSource | Self::BitSource | Self::IoSource | Self::IoSink => "Sources / HAL",
            Self::ConvEncode
            | Self::ConvDecode
            | Self::RsEncode
            | Self::RsDecode
            | Self::LdpcEncode
            | Self::LdpcDecode
            | Self::TurboEncode
            | Self::TurboDecode => "Codage canal",
            Self::DigitalMod
            | Self::DigitalDemod
            | Self::AnalogMod
            | Self::AnalogDemod
            | Self::Pll
            | Self::Costas
            | Self::SymbolTiming
            | Self::FrameSync => "Modulation / synchronisation",
            Self::Power
            | Self::Snr
            | Self::Thd
            | Self::Evm
            | Self::Ber
            | Self::Per
            | Self::PhaseNoise
            | Self::Vswr
            | Self::Psd
            | Self::Spectrogram
            | Self::SpectralPeak => "Mesures DSP",
            Self::DcOffset
            | Self::IqBalance
            | Self::Cable
            | Self::CalibrationTable
            | Self::Uncertainty
            | Self::Convert => "Calibration / unités",
            _ => "DSP / canal",
        }
    }
    pub fn inputs(self) -> &'static [super::Terminal] {
        use super::{Port::*, Terminal};
        const IQ: Terminal = Terminal {
            name: "IQ",
            port: ComplexIq,
            required: true,
        };
        const BITS: Terminal = Terminal {
            name: "BITS",
            port: Bits,
            required: true,
        };
        const Q: Terminal = Terminal {
            name: "VALUE",
            port: Quantity,
            required: true,
        };
        match self {
            Self::IqSource | Self::BitSource | Self::IoSource => &[],
            Self::ConvEncode
            | Self::ConvDecode
            | Self::RsEncode
            | Self::RsDecode
            | Self::LdpcEncode
            | Self::LdpcDecode
            | Self::TurboEncode
            | Self::TurboDecode
            | Self::DigitalMod
            | Self::FrameSync => &[BITS],
            Self::Ber | Self::Per => &[
                BITS,
                Terminal {
                    name: "REF",
                    port: Bits,
                    required: true,
                },
            ],
            Self::Evm | Self::Snr => &[
                IQ,
                Terminal {
                    name: "REF",
                    port: ComplexIq,
                    required: true,
                },
            ],
            Self::Convert | Self::Uncertainty => &[Q],
            Self::SpectralPeak => &[Terminal {
                name: "PSD",
                port: Spectrum,
                required: true,
            }],
            Self::Vswr => &[Terminal {
                name: "S11/S22",
                port: SParameters,
                required: true,
            }],
            _ => &[IQ],
        }
    }
    pub fn outputs(self) -> &'static [super::Terminal] {
        use super::{Port::*, Terminal};
        let p = match self {
            Self::BitSource
            | Self::DigitalDemod
            | Self::FrameSync
            | Self::ConvEncode
            | Self::ConvDecode
            | Self::RsEncode
            | Self::RsDecode
            | Self::LdpcEncode
            | Self::LdpcDecode
            | Self::TurboEncode
            | Self::TurboDecode => Bits,
            Self::Power
            | Self::Snr
            | Self::Thd
            | Self::Evm
            | Self::Ber
            | Self::Per
            | Self::Vswr
            | Self::SpectralPeak
            | Self::Uncertainty
            | Self::Convert => Quantity,
            Self::Psd | Self::Spectrogram | Self::Fft | Self::PhaseNoise => Spectrum,
            _ => ComplexIq,
        };
        match p {
            Bits => &[Terminal {
                name: "BITS",
                port: Bits,
                required: false,
            }],
            Quantity => &[Terminal {
                name: "VALUE",
                port: Quantity,
                required: false,
            }],
            Spectrum => &[Terminal {
                name: "SPECTRUM",
                port: Spectrum,
                required: false,
            }],
            _ => &[Terminal {
                name: "IQ",
                port: ComplexIq,
                required: false,
            }],
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub rate: f64,
    pub center_hz: f64,
    pub samples: usize,
    pub tone_hz: f64,
    pub amplitude: f64,
    pub factor: usize,
    pub taps: Vec<f64>,
    pub denominator: Vec<f64>,
    pub fft_size: usize,
    pub window: String,
    pub modulation: String,
    pub order: usize,
    pub sps: usize,
    pub noise_db: f64,
    pub seed: u64,
    pub bandwidth: f64,
    pub phase: f64,
    pub frequency_offset: f64,
    pub multipath: Vec<Complex>,
    pub fading: bool,
    pub nonlinearity: f64,
    pub dc: Complex,
    pub iq_gain: f64,
    pub iq_phase: f64,
    pub cable_db: f64,
    pub volts_per_fs: Option<f64>,
    pub delay_s: f64,
    pub calibration: Vec<[f64; 3]>,
    pub uncertainties: Vec<f64>,
    pub coverage: f64,
    pub impedance: f64,
    pub target_unit: Unit,
    pub packet_bits: usize,
    pub preamble: Vec<u8>,
    pub iterations: usize,
    pub rs_parity: usize,
    pub rx_scale: f64,
    pub tx_scale: f64,
    pub io_backend: String,
    pub endpoint: String,
    pub format: String,
    pub scpi_query: String,
    pub python_executable: String,
    pub timeout_ms: u64,
    pub reconnects: usize,
    pub clock: String,
    pub trigger: String,
    pub mimo_channels: usize,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            rate: 48000.,
            center_hz: 100e6,
            samples: 1024,
            tone_hz: 3000.,
            amplitude: 1.,
            factor: 2,
            taps: vec![0.25, 0.5, 0.25],
            denominator: vec![1., -0.5],
            fft_size: 256,
            window: "Hann".into(),
            modulation: "QAM".into(),
            order: 16,
            sps: 4,
            noise_db: 30.,
            seed: 42,
            bandwidth: 0.01,
            phase: 0.,
            frequency_offset: 0.,
            multipath: vec![Complex::new(1., 0.)],
            fading: false,
            nonlinearity: 0.,
            dc: Complex::default(),
            iq_gain: 1.,
            iq_phase: 0.,
            cable_db: 0.,
            volts_per_fs: None,
            delay_s: 0.,
            calibration: vec![[1e6, 0., 0.], [6e9, 0., 0.]],
            uncertainties: vec![0.01, 0.02],
            coverage: 2.,
            impedance: 50.,
            target_unit: Unit::Dbm,
            packet_bits: 64,
            preamble: vec![1, 0, 1, 0, 1, 1, 0, 1],
            iterations: 8,
            rs_parity: 16,
            rx_scale: 2048.,
            tx_scale: 16384.,
            io_backend: "RAW".into(),
            endpoint: "recording.cf32".into(),
            format: "cf32_le".into(),
            scpi_query: ":READ:IQ?".into(),
            python_executable: "python".into(),
            timeout_ms: 2000,
            reconnects: 2,
            clock: "internal".into(),
            trigger: "immediate".into(),
            mimo_channels: 1,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        let finite = [
            self.rx_scale,
            self.tx_scale,
            self.rate,
            self.center_hz,
            self.tone_hz,
            self.amplitude,
            self.noise_db,
            self.bandwidth,
            self.phase,
            self.frequency_offset,
            self.nonlinearity,
            self.dc.re,
            self.dc.im,
            self.iq_gain,
            self.iq_phase,
            self.cable_db,
            self.delay_s,
            self.coverage,
            self.impedance,
        ];
        if finite
            .iter()
            .chain(&self.taps)
            .chain(&self.denominator)
            .chain(&self.uncertainties)
            .any(|x| !x.is_finite())
            || self.rx_scale <= 0.
            || self.tx_scale <= 0.
            || self.rate <= 0.
            || self.center_hz < 0.
            || self.amplitude <= 0.
            || self.volts_per_fs.is_some_and(|v| !v.is_finite() || v <= 0.)
            || self.nonlinearity < 0.
            || self.noise_db.abs() > 300.
            || self.amplitude > 1e12
            || self.uncertainties.len() > 1024
            || !(1..=65536).contains(&self.samples)
            || !(1..=64).contains(&self.factor)
            || !self.fft_size.is_power_of_two()
            || !(8..=8192).contains(&self.fft_size)
            || self.taps.is_empty()
            || self.taps.len() > 1024
            || self.denominator.is_empty()
            || self.denominator.len() > 128
            || self.denominator[0].abs() < 1e-12
            || !(2..=256).contains(&self.order)
            || !(1..=128).contains(&self.sps)
            || !(1..=32).contains(&self.iterations)
            || !(2..=64).contains(&self.rs_parity)
            || self.packet_bits == 0
            || self.packet_bits > 65536
            || self.preamble.is_empty()
            || self.preamble.len() > 1024
            || self.preamble.iter().any(|b| *b > 1)
            || !(0.000001..=0.2).contains(&self.bandwidth)
            || self.impedance <= 0.
            || self.coverage <= 0.
            || self.iq_gain <= 0.
            || self.iq_phase.cos().abs() < 0.01
            || self.uncertainties.iter().any(|x| *x < 0.)
            || self.multipath.is_empty()
            || self.multipath.len() > 128
            || self
                .multipath
                .iter()
                .any(|z| !z.re.is_finite() || !z.im.is_finite())
            || self.calibration.len() < 2
            || self.calibration.len() > 1000
            || self.calibration.iter().flatten().any(|x| !x.is_finite())
            || self.calibration.windows(2).any(|w| w[1][0] <= w[0][0])
            || self.endpoint.len() > 4096
            || !(1..=30000).contains(&self.timeout_ms)
            || self.reconnects > 5
            || !(1..=8).contains(&self.mimo_channels)
        {
            return Err("Paramètres RF/DSP invalides ou hors limites".into());
        }
        Ok(())
    }
}
