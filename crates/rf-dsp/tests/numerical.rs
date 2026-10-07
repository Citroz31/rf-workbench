use rf_core::dsp::*;
use rf_dsp::{Processor, fec, fft, modem};
fn settings() -> Settings {
    Settings::default()
}
fn frame(samples: Vec<Complex>) -> IqFrame {
    IqFrame {
        samples,
        sample_rate: 48000.,
        center_hz: 100e6,
        unit: Unit::Volt,
        simulated: true,
        time: TimeTag::default(),
    }
}
fn bits(n: usize) -> BitFrame {
    BitFrame {
        bits: (0..n).map(|i| ((i * 7 + i / 5) % 2) as u8).collect(),
        llr: vec![],
        simulated: true,
    }
}
fn iq(d: Data) -> IqFrame {
    let Data::Iq(f) = d else {
        panic!("IQ expected")
    };
    f
}
fn quantity(d: Data) -> f64 {
    let Data::Quantity { value, .. } = d else {
        panic!("quantity expected")
    };
    value
}
#[test]
fn fft_known_bin_and_inverse() {
    let mut z: Vec<_> = (0..256)
        .map(|i| Complex::polar(1., std::f64::consts::TAU * 7. * i as f64 / 256.))
        .collect();
    let original = z.clone();
    fft(&mut z, false).unwrap();
    assert!((z[7].re - 256.).abs() < 1e-9);
    assert!(
        z.iter()
            .enumerate()
            .filter(|(i, _)| *i != 7)
            .all(|(_, v)| v.norm2() < 1e-18)
    );
    fft(&mut z, true).unwrap();
    for (a, b) in z.iter().zip(original) {
        assert!((*a - b).norm2() < 1e-20)
    }
    assert!(fft(&mut [Complex::new(1., 0.)], false).is_ok());
}
#[test]
fn welch_density_integrates_to_power() {
    let f = frame(
        (0..1024)
            .map(|i| Complex::polar(2., std::f64::consts::TAU * 3000. * i as f64 / 48000.))
            .collect(),
    );
    let Data::Spectrum(s) = Processor::default()
        .process(1, Op::Psd, &settings(), &[Data::Iq(f)], 0)
        .unwrap()
    else {
        panic!()
    };
    let integral = s.levels.iter().map(|db| 10f64.powf(db / 10.)).sum::<f64>() * 48000. / 256.;
    assert!((integral - 4.).abs() < 1e-8);
    assert_eq!(s.unit, Unit::DbVolt2PerHz);
}
#[test]
fn fir_chunk_boundary_and_decimation() {
    let c = Settings {
        taps: vec![0.2, 0.3, 0.5],
        ..settings()
    };
    let f = frame(
        (0..100)
            .map(|i| Complex::new(i as f64, -(i as f64)))
            .collect(),
    );
    let whole = iq(Processor::default()
        .process(1, Op::Fir, &c, &[Data::Iq(f.clone())], 0)
        .unwrap());
    let mut p = Processor::default();
    let a = IqFrame {
        samples: f.samples[..37].to_vec(),
        ..f.clone()
    };
    let b = IqFrame {
        samples: f.samples[37..].to_vec(),
        time: TimeTag {
            first_sample: 37,
            ..Default::default()
        },
        ..f
    };
    let mut split = iq(p.process(1, Op::Fir, &c, &[Data::Iq(a)], 0).unwrap()).samples;
    split.extend(iq(p.process(1, Op::Fir, &c, &[Data::Iq(b)], 1).unwrap()).samples);
    assert_eq!(split, whole.samples);
    let dec = iq(Processor::default()
        .process(2, Op::Decimate, &c, &[Data::Iq(whole)], 0)
        .unwrap());
    assert_eq!(dec.samples.len(), 50);
    assert_eq!(dec.sample_rate, 24000.);
}
#[test]
fn digital_modems_roundtrip() {
    for (mode, order, n, sps, tone) in [
        ("ASK", 4, 512, 4, 3000.),
        ("PSK", 8, 768, 4, 3000.),
        ("QAM", 16, 1024, 4, 3000.),
        ("QAM", 64, 768, 4, 3000.),
        ("QAM", 256, 1024, 4, 3000.),
        ("OFDM", 16, 1024, 4, 3000.),
        ("FSK", 4, 512, 32, 750.),
    ] {
        let c = Settings {
            modulation: mode.into(),
            order,
            sps,
            tone_hz: tone,
            ..settings()
        };
        let b = bits(n);
        let f = modem::modulate(&b, &c).unwrap();
        let decoded = modem::demodulate(&f, &c).unwrap();
        assert_eq!(b.bits, decoded.bits, "{mode}/{order}");
        assert_eq!(decoded.llr.len(), n);
    }
}
#[test]
fn convolutional_corrects_single_error() {
    let b = bits(256);
    let mut encoded = fec::conv_encode(&b.bits);
    encoded[55] ^= 1;
    let llr: Vec<_> = encoded
        .iter()
        .map(|x| if *x == 0 { 8. } else { -8. })
        .collect();
    assert_eq!(fec::viterbi(&llr).unwrap(), b.bits);
}
#[test]
fn reed_solomon_corrects_bounded_errors() {
    let data: Vec<u8> = (0..239).map(|i| (i * 17) as u8).collect();
    let original = fec::rs_encode(&data, 16).unwrap();
    for count in [0, 1, 4, 8] {
        let mut encoded = original.clone();
        for i in 0..count {
            encoded[i * 23] ^= (i + 13) as u8;
        }
        let corrected = fec::rs_decode(&mut encoded, 16).unwrap();
        assert_eq!(&encoded[..239], data);
        assert_eq!(corrected, count);
    }
}
#[test]
fn fec_reference_profiles_roundtrip() {
    let mut p = Processor::default();
    for (enc, dec, n) in [
        (Op::LdpcEncode, Op::LdpcDecode, 128),
        (Op::TurboEncode, Op::TurboDecode, 256),
    ] {
        let b = bits(n);
        let encoded = p
            .process(1, enc, &settings(), &[Data::Bits(b.clone())], 0)
            .unwrap();
        let decoded = p.process(2, dec, &settings(), &[encoded], 0).unwrap();
        let Data::Bits(d) = decoded else { panic!() };
        assert_eq!(d.bits, b.bits, "{enc:?}");
    }
}
#[test]
fn awgn_snr_evm_and_unit_conversion() {
    let mut p = Processor::default();
    let c = Settings {
        samples: 65536,
        noise_db: 20.,
        ..settings()
    };
    let original = p.process(1, Op::IqSource, &c, &[], 0).unwrap();
    let noisy = p
        .process(2, Op::Channel, &c, std::slice::from_ref(&original), 0)
        .unwrap();
    let snr = quantity(
        p.process(3, Op::Snr, &c, &[noisy.clone(), original.clone()], 0)
            .unwrap(),
    );
    let evm = quantity(p.process(4, Op::Evm, &c, &[noisy, original], 0).unwrap());
    assert!((snr - 20.).abs() < 0.2);
    assert!((evm - 10.).abs() < 0.3);
    assert!((convert(0., Unit::Dbm, Unit::Volt, 50.).unwrap() - 0.22360679775).abs() < 1e-10);
    assert!(convert(1., Unit::Hz, Unit::Second, 50.).is_err());
}
#[test]
fn calibration_matrix_and_invalid_references() {
    let mut p = Processor::default();
    let c = Settings {
        iq_gain: 1.2,
        iq_phase: 0.1,
        dc: Complex::new(0.1, -0.2),
        ..settings()
    };
    let distorted = frame(vec![
        Complex::new(
            0.3 + c.dc.re,
            1.2 * (0.4 * 0.1f64.cos() + 0.3 * 0.1f64.sin()) + c.dc.im
        );
        256
    ]);
    let balanced = iq(p
        .process(1, Op::IqBalance, &c, &[Data::Iq(distorted)], 0)
        .unwrap());
    assert!((balanced.samples[0].re - 0.3).abs() < 1e-12);
    assert!((balanced.samples[0].im - 0.4).abs() < 1e-12);
    let zero = Data::Iq(frame(vec![Complex::default(); 256]));
    assert!(p.process(2, Op::Snr, &c, &[zero.clone(), zero], 0).is_err());
}
#[test]
fn timestamps_reject_overflow() {
    let mut f = frame(vec![Complex::new(1., 0.); 16]);
    f.time.first_sample = u64::MAX;
    assert!(f.validate().is_err());
}
#[test]
fn gray_neighbors_and_public_modem_limits() {
    let c = Settings {
        modulation: "ASK".into(),
        order: 16,
        sps: 1,
        ..settings()
    };
    let mut positions: Vec<_> = (0..16usize)
        .map(|label| {
            let bits = BitFrame {
                bits: (0..4).rev().map(|i| ((label >> i) & 1) as u8).collect(),
                llr: vec![],
                simulated: true,
            };
            let value = modem::modulate(&bits, &c).unwrap().samples[0].re;
            (value, label)
        })
        .collect();
    positions.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert!(
        positions
            .windows(2)
            .all(|p| (p[0].1 ^ p[1].1).count_ones() == 1)
    );
    let c = Settings {
        order: 1,
        ..settings()
    };
    assert!(modem::modulate(&bits(16), &c).is_err());
    let c = Settings {
        sps: 128,
        ..settings()
    };
    assert!(modem::modulate(&bits(65536), &c).is_err());
}
#[test]
fn analog_modems_and_coherent_thd() {
    let original = frame(
        (0..1024)
            .map(|i| {
                Complex::new(
                    0.2 * (std::f64::consts::TAU * 1500. * i as f64 / 48000.).sin(),
                    0.,
                )
            })
            .collect(),
    );
    for mode in ["AM", "FM", "PM"] {
        let c = Settings {
            modulation: mode.into(),
            tone_hz: 3000.,
            amplitude: 0.5,
            ..settings()
        };
        let mut p = Processor::default();
        let encoded = p
            .process(1, Op::AnalogMod, &c, &[Data::Iq(original.clone())], 0)
            .unwrap();
        let decoded = iq(p.process(2, Op::AnalogDemod, &c, &[encoded], 0).unwrap());
        for (a, b) in decoded
            .samples
            .iter()
            .skip(1)
            .zip(original.samples.iter().skip(1))
        {
            assert!((a.re - b.re).abs() < 1e-10, "{mode}")
        }
    }
    let c = Settings {
        tone_hz: 1500.,
        ..settings()
    };
    let distorted = frame(
        (0..1024)
            .map(|i| {
                let phase = std::f64::consts::TAU * 1500. * i as f64 / 48000.;
                Complex::polar(1., phase) + Complex::polar(0.1, 2. * phase)
            })
            .collect(),
    );
    let thd = quantity(
        Processor::default()
            .process(1, Op::Thd, &c, &[Data::Iq(distorted)], 0)
            .unwrap(),
    );
    assert!((thd - 10.).abs() < 1e-8);
}
#[test]
fn phase_noise_offset_axis_and_sync() {
    let c = Settings::default();
    let f = frame(
        (0..4096)
            .map(|i| {
                Complex::polar(
                    1.,
                    std::f64::consts::TAU * 3000. * i as f64 / 48000.
                        + 0.01 * (std::f64::consts::TAU * 1500. * i as f64 / 48000.).sin(),
                )
            })
            .collect(),
    );
    let Data::Spectrum(s) = Processor::default()
        .process(1, Op::PhaseNoise, &c, &[Data::Iq(f)], 0)
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(s.unit, Unit::DbcPerHz);
    assert!(s.frequency_hz.iter().all(|x| *x > 0.));
    let peak = s
        .levels
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap()
        .0;
    assert_eq!(s.frequency_hz[peak], 1500.);
    let c = Settings {
        packet_bits: 16,
        ..settings()
    };
    let mut input = vec![0, 0, 0];
    input.extend(&c.preamble);
    input.extend(&bits(16).bits);
    let d = Processor::default()
        .process(
            1,
            Op::FrameSync,
            &c,
            &[Data::Bits(BitFrame {
                bits: input,
                llr: vec![],
                simulated: true,
            })],
            0,
        )
        .unwrap();
    let Data::Bits(d) = d else { panic!() };
    assert_eq!(d.bits, bits(16).bits);
}
#[test]
fn si_prefixes_and_dimensions() {
    assert_eq!(parse_quantity("2.45 GHz").unwrap(), (2.45e9, Unit::Hz));
    assert_eq!(parse_quantity("250 mV").unwrap(), (0.25, Unit::Volt));
    assert_eq!(parse_quantity("1 µs").unwrap(), (1e-6, Unit::Second));
    assert!((convert(0., Unit::Dbv, Unit::Volt, 50.).unwrap() - 1.).abs() < 1e-12);
    assert!(parse_quantity("NaN Hz").is_err());
}
#[test]
fn integrated_scientific_suite() {
    for t in rf_dsp::self_tests() {
        assert!(t.passed, "{}", t.name);
    }
}
#[test]
fn ldpc_and_turbo_use_soft_error_correction() {
    let original = bits(128);
    for (encode, decode) in [
        (Op::LdpcEncode, Op::LdpcDecode),
        (Op::TurboEncode, Op::TurboDecode),
    ] {
        let c = settings();
        let mut f = fec::process(encode, &original, &c).unwrap();
        f.llr = f
            .bits
            .iter()
            .map(|b| if *b == 0 { 8. } else { -8. })
            .collect();
        f.bits[17] ^= 1;
        f.llr[17] *= -1.;
        let decoded = fec::process(decode, &f, &c).unwrap();
        assert_eq!(decoded.bits, original.bits, "{encode:?}");
    }
}
#[test]
fn explicit_full_scale_calibration_enables_power_measurement() {
    let input = Data::Iq(IqFrame {
        unit: Unit::Fs,
        ..frame(vec![Complex::new(0.5, 0.); 256])
    });
    let c = Settings {
        volts_per_fs: Some(2.),
        ..settings()
    };
    let mut p = Processor::default();
    assert!(
        p.process(1, Op::Power, &c, std::slice::from_ref(&input), 0)
            .is_err()
    );
    let calibrated = p.process(2, Op::CalibrationTable, &c, &[input], 0).unwrap();
    let power = quantity(p.process(3, Op::Power, &c, &[calibrated], 0).unwrap());
    assert!((power - 13.01029995664).abs() < 1e-9);
}
#[test]
fn generated_sdr_samples_have_explicit_full_scale_units() {
    let c = Settings {
        source_unit: Unit::Fs,
        amplitude: 0.5,
        ..settings()
    };
    let generated = iq(Processor::default()
        .process(1, Op::IqSource, &c, &[], 0)
        .unwrap());
    assert_eq!(generated.unit, Unit::Fs);
    assert!(generated.samples.iter().all(|z| z.norm2() <= 1.));
    let modulated = modem::modulate(&bits(1024), &c).unwrap();
    assert_eq!(modulated.unit, Unit::Fs);
    assert!(modulated.samples.iter().all(|z| z.norm2() <= 1.));
    let s = rf_dsp::psd(&modulated, &c, false).unwrap();
    assert_eq!(s.unit, Unit::DbFs2PerHz);
}
