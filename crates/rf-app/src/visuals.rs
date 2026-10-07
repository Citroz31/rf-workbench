//! Resolution-independent instrument illustrations and electronic symbols.
use crate::theme::*;
use eframe::egui::{self, Align2, FontId, Pos2, Rect, Stroke, StrokeKind};
use rf_core::Kind;

pub fn symbol(p: &egui::Painter, rect: Rect, kind: Kind) {
    symbol_with_ports(p, rect, kind, 2);
}
pub fn symbol_with_ports(p: &egui::Painter, rect: Rect, kind: Kind, port_count: u8) {
    let at = |x: f32, y: f32| rect.min + egui::vec2(x * rect.width(), y * rect.height());
    let line = |a: (f32, f32), b: (f32, f32), color| {
        p.line_segment(
            [at(a.0, a.1), at(b.0, b.1)],
            Stroke::new((rect.height() / 50.).clamp(1., 2.), color),
        );
    };
    let text = |x: f32, y: f32, t: &str, color| {
        p.text(
            at(x, y),
            Align2::CENTER_CENTER,
            t,
            FontId::monospace((rect.height() * 0.19).clamp(7., 16.)),
            color,
        );
    };
    let accent = crate::theme::kind(kind);
    match kind {
        Kind::Dsp(op) => {
            use rf_core::dsp::Op;
            p.rect_stroke(
                rect.shrink(4.),
                6.,
                Stroke::new(2., accent),
                StrokeKind::Inside,
            );
            match op {
                Op::DigitalMod | Op::DigitalDemod | Op::Evm | Op::IqBalance => {
                    for x in 0..4 {
                        for y in 0..4 {
                            p.circle_filled(
                                at(0.32 + x as f32 * 0.12, 0.23 + y as f32 * 0.12),
                                (rect.height() * 0.035).max(1.),
                                accent,
                            );
                        }
                    }
                    text(0.5, 0.84, "I / Q", muted());
                }
                Op::Fft | Op::Psd | Op::Spectrogram | Op::PhaseNoise | Op::SpectralPeak => {
                    for i in 0..13 {
                        let x = 0.2 + i as f32 * 0.05;
                        let h = 0.18 + 0.45 * (-((i as f32 - 6.) / 2.).powi(2)).exp();
                        line(
                            (x, 0.76),
                            (x, 0.76 - h),
                            if i == 6 { gold() } else { accent },
                        );
                    }
                    text(0.5, 0.9, "f", muted());
                }
                Op::Fir | Op::Iir | Op::Decimate | Op::Interpolate | Op::Window => {
                    let points = (0..36)
                        .map(|i| {
                            let x = i as f32 / 35.;
                            at(
                                0.2 + 0.6 * x,
                                0.25 + 0.45 / (1. + (-15. * (x - 0.55)).exp()),
                            )
                        })
                        .collect();
                    p.add(egui::Shape::line(points, Stroke::new(2., accent)));
                    text(
                        0.5,
                        0.86,
                        op.label().split_whitespace().next().unwrap_or("DSP"),
                        muted(),
                    );
                }
                Op::ConvEncode
                | Op::ConvDecode
                | Op::RsEncode
                | Op::RsDecode
                | Op::LdpcEncode
                | Op::LdpcDecode
                | Op::TurboEncode
                | Op::TurboDecode
                | Op::BitSource
                | Op::FrameSync => {
                    text(0.5, 0.3, "0101 1100", accent);
                    for i in 0..5 {
                        line(
                            (0.25 + i as f32 * 0.12, 0.46),
                            (0.25 + i as f32 * 0.12, 0.72),
                            muted(),
                        );
                    }
                    text(0.5, 0.85, "FEC / BITS", muted());
                }
                Op::IoSource | Op::IoSink => {
                    p.rect_filled(
                        Rect::from_min_max(at(0.3, 0.22), at(0.7, 0.65)),
                        4.,
                        border(),
                    );
                    for i in 0..3 {
                        p.circle_filled(
                            at(0.39 + i as f32 * 0.11, 0.45),
                            rect.height() * 0.05,
                            accent,
                        );
                    }
                    text(0.5, 0.84, "HAL", accent);
                }
                Op::Power
                | Op::Snr
                | Op::Thd
                | Op::Ber
                | Op::Per
                | Op::Vswr
                | Op::Uncertainty
                | Op::Convert => {
                    p.circle_stroke(
                        at(0.5, 0.48),
                        rect.height() * 0.24,
                        Stroke::new(1.5, accent),
                    );
                    line((0.5, 0.48), (0.62, 0.3), gold());
                    text(
                        0.5,
                        0.87,
                        op.label().split_whitespace().next().unwrap_or("DSP"),
                        muted(),
                    );
                }
                _ => {
                    let points = (0..48)
                        .map(|i| {
                            let x = i as f32 / 47.;
                            at(
                                0.18 + x * 0.64,
                                0.45 + 0.19 * (x * std::f32::consts::TAU * 2.).sin(),
                            )
                        })
                        .collect();
                    p.add(egui::Shape::line(points, Stroke::new(2., accent)));
                    text(
                        0.5,
                        0.86,
                        op.label().split_whitespace().next().unwrap_or("DSP"),
                        muted(),
                    );
                }
            }
            line((0., 0.5), (0.08, 0.5), accent);
            line((0.92, 0.5), (1., 0.5), accent);
        }
        Kind::DcSupplyE3631A | Kind::DcSupplyE36313A => {
            let chassis = Rect::from_min_max(at(0.03, 0.06), at(0.97, 0.95));
            p.rect_filled(chassis, 5., egui::Color32::from_rgb(63, 78, 94));
            p.rect_stroke(chassis, 5., Stroke::new(1., muted()), StrokeKind::Inside);
            for i in 0..3 {
                let x = 0.08 + i as f32 * 0.3;
                p.rect_filled(
                    Rect::from_min_max(at(x, 0.16), at(x + 0.25, 0.57)),
                    3.,
                    bg(),
                );
                text(x + 0.125, 0.3, &format!("CH{}", i + 1), teal());
                text(x + 0.125, 0.48, "V / A", muted());
                for (dx, color) in [(0.07, red()), (0.18, blue())] {
                    p.circle_filled(at(x + dx, 0.78), rect.height() * 0.065, color);
                    p.circle_stroke(
                        at(x + dx, 0.78),
                        rect.height() * 0.065,
                        Stroke::new(1., text_color()),
                    );
                }
            }
        }
        Kind::Generator
        | Kind::Analyzer
        | Kind::Pna
        | Kind::PnaX
        | Kind::UsbVna
        | Kind::Awg
        | Kind::NoiseFigureMeter
        | Kind::PowerMeter
        | Kind::Thermostream => {
            let chassis = Rect::from_min_max(at(0.03, 0.06), at(0.97, 0.92));
            p.rect_filled(chassis, 5., egui::Color32::from_rgb(63, 78, 94));
            p.rect_stroke(chassis, 5., Stroke::new(1., muted()), StrokeKind::Inside);
            let screen = Rect::from_min_max(at(0.09, 0.18), at(0.65, 0.72));
            p.rect_filled(screen, 3., bg());
            for n in 1..5 {
                let x = 0.09 + 0.56 * n as f32 / 5.;
                line((x, 0.18), (x, 0.72), border());
            }
            for n in 1..4 {
                let y = 0.18 + 0.54 * n as f32 / 4.;
                line((0.09, y), (0.65, y), border());
            }
            if matches!(
                kind,
                Kind::PowerMeter | Kind::NoiseFigureMeter | Kind::Thermostream
            ) {
                text(
                    0.37,
                    0.44,
                    match kind {
                        Kind::PowerMeter => "-13 dBm",
                        Kind::Thermostream => "25 °C",
                        _ => "NF 2.5",
                    },
                    accent,
                );
            } else {
                let points: Vec<_> = (0..50)
                    .map(|i| {
                        let t = i as f32 / 49.;
                        let y = match kind {
                            Kind::Analyzer => 0.66 - 0.43 * (-((t - 0.55) * 8.).powi(2)).exp(),
                            Kind::Pna | Kind::PnaX | Kind::UsbVna => {
                                0.43 + 0.07 * (t * std::f32::consts::TAU).sin()
                            }
                            _ => 0.45 + 0.18 * (t * std::f32::consts::TAU * 2.).sin(),
                        };
                        at(0.11 + t * 0.52, y)
                    })
                    .collect();
                p.add(egui::Shape::line(points, Stroke::new(1.6, accent)));
                if kind == Kind::PnaX {
                    line((0.11, 0.3), (0.63, 0.63), gold());
                }
            }
            p.circle_filled(at(0.81, 0.42), rect.height() * 0.14, card_fill());
            p.circle_stroke(
                at(0.81, 0.42),
                rect.height() * 0.14,
                Stroke::new(1., muted()),
            );
            line((0.81, 0.42), (0.85, 0.32), text_color());
            for x in [0.74, 0.82, 0.9] {
                for y in [0.66, 0.77] {
                    p.rect_filled(
                        Rect::from_center_size(
                            at(x, y),
                            egui::vec2(rect.width() * 0.045, rect.height() * 0.055),
                        ),
                        1.,
                        muted(),
                    );
                }
            }
            let count = if kind.max_rf_ports().is_some() {
                port_count.clamp(1, kind.max_rf_ports().unwrap())
            } else {
                2
            };
            for i in 0..count {
                p.circle_stroke(
                    at(0.12 + i as f32 * 0.12, 0.84),
                    rect.height() * 0.045,
                    Stroke::new(1.3, gold()),
                );
            }
        }
        Kind::Dut => {
            let chip = Rect::from_min_max(at(0.30, 0.14), at(0.70, 0.86));
            p.rect_filled(chip, 5., card_fill());
            p.rect_stroke(chip, 5., Stroke::new(1.8, accent), StrokeKind::Inside);
            for n in 0..5 {
                let y = 0.23 + n as f32 * 0.13;
                line((0.20, y), (0.30, y), gold());
                line((0.70, y), (0.80, y), gold());
            }
            text(0.5, 0.5, "DUT", accent);
        }
        Kind::IqModulator => {
            for (y, label) in [(0.27, "I"), (0.73, "Q")] {
                line((0.04, y), (0.27, y), blue());
                p.circle_stroke(at(0.35, y), rect.height() * 0.13, Stroke::new(1.5, accent));
                line((0.31, y - 0.07), (0.39, y + 0.07), accent);
                line((0.31, y + 0.07), (0.39, y - 0.07), accent);
                line((0.43, y), (0.64, y), accent);
                line((0.64, y), (0.64, 0.5), accent);
                text(0.10, y - 0.14, label, blue());
            }
            line((0.35, 0.42), (0.35, 0.58), gold());
            line((0.64, 0.5), (0.95, 0.5), gold());
            text(0.87, 0.27, "RF", gold());
            text(0.49, 0.5, "LO", gold());
        }
        Kind::Dac | Kind::Adc => {
            let points = if kind == Kind::Dac {
                vec![
                    at(0.23, 0.15),
                    at(0.64, 0.15),
                    at(0.80, 0.50),
                    at(0.64, 0.85),
                    at(0.23, 0.85),
                ]
            } else {
                vec![
                    at(0.20, 0.50),
                    at(0.36, 0.15),
                    at(0.77, 0.15),
                    at(0.77, 0.85),
                    at(0.36, 0.85),
                ]
            };
            p.add(egui::Shape::convex_polygon(
                points,
                card_fill(),
                Stroke::new(1.7, accent),
            ));
            line((0.03, 0.5), (0.20, 0.5), blue());
            line((0.80, 0.5), (0.97, 0.5), teal());
            text(
                0.5,
                0.38,
                if kind == Kind::Dac { "DAC" } else { "CAN" },
                accent,
            );
            text(
                0.5,
                0.65,
                if kind == Kind::Dac { "010 ~" } else { "~ 010" },
                muted(),
            );
        }
        Kind::VariableResistor => {
            line((0.05, 0.52), (0.25, 0.52), gold());
            line((0.75, 0.52), (0.95, 0.52), gold());
            p.rect_stroke(
                Rect::from_min_max(at(0.25, 0.34), at(0.75, 0.7)),
                0.,
                Stroke::new(1.7, accent),
                StrokeKind::Inside,
            );
            line((0.34, 0.88), (0.67, 0.08), text_color());
            line((0.67, 0.08), (0.57, 0.18), text_color());
            line((0.67, 0.08), (0.68, 0.27), text_color());
        }
        Kind::Thermometer => {
            p.rect_stroke(
                Rect::from_min_max(at(0.43, 0.10), at(0.57, 0.7)),
                6.,
                Stroke::new(1.7, accent),
                StrokeKind::Inside,
            );
            p.circle_filled(at(0.5, 0.73), rect.height() * 0.17, accent);
            line((0.5, 0.25), (0.5, 0.7), accent);
            for n in 0..4 {
                line(
                    (0.58, 0.18 + n as f32 * 0.12),
                    (0.66, 0.18 + n as f32 * 0.12),
                    muted(),
                );
            }
            text(0.80, 0.5, "°C", accent);
        }
        Kind::PowerSensor => {
            p.rect_filled(
                Rect::from_min_max(at(0.25, 0.30), at(0.68, 0.75)),
                7.,
                egui::Color32::from_rgb(76, 85, 97),
            );
            p.rect_stroke(
                Rect::from_min_max(at(0.13, 0.40), at(0.25, 0.65)),
                1.,
                Stroke::new(2., gold()),
                StrokeKind::Inside,
            );
            line((0.68, 0.52), (0.87, 0.52), text_color());
            line((0.87, 0.52), (0.93, 0.8), text_color());
            text(0.46, 0.51, "RF", accent);
        }
        Kind::Python => {
            text(0.5, 0.5, "{ Py }", purple());
            line((0.17, 0.17), (0.83, 0.17), border());
            line((0.17, 0.83), (0.83, 0.83), border());
        }
        Kind::Peak => {
            line((0.05, 0.85), (0.95, 0.85), muted());
            line((0.1, 0.85), (0.1, 0.12), muted());
            let pts: Vec<Pos2> = (0..35)
                .map(|i| {
                    let t = i as f32 / 34.;
                    at(
                        0.1 + 0.84 * t,
                        0.8 - 0.6 * (-((t - 0.5) * 6.).powi(2)).exp(),
                    )
                })
                .collect();
            p.add(egui::Shape::line(pts, Stroke::new(1.8, accent)));
            p.circle_filled(at(0.52, 0.2), 3., gold());
        }
        Kind::Limit => {
            line((0.1, 0.26), (0.9, 0.26), gold());
            line((0.1, 0.74), (0.9, 0.74), gold());
            line((0.3, 0.5), (0.45, 0.64), teal());
            line((0.45, 0.64), (0.72, 0.34), teal());
        }
    }
}

#[derive(Clone, Copy)]
pub enum Icon {
    Select,
    Wire,
    Pan,
    Fit,
    Save,
    Open,
    Undo,
    Redo,
    Play,
    Stop,
    Network,
    Wave,
    Chip,
    Test,
    Python,
    Instrument,
    Settings,
    Export,
    Grid,
    Clone,
}
pub fn icon_button(ui: &mut egui::Ui, icon: Icon, tooltip: &str, selected: bool) -> egui::Response {
    let (r, response) = ui.allocate_exact_size(egui::vec2(32., 30.), egui::Sense::click());
    let p = ui.painter();
    let color = if selected { teal() } else { text_color() };
    p.rect_filled(
        r,
        5.,
        if selected {
            egui::Color32::from_rgb(26, 75, 73)
        } else if response.hovered() {
            border()
        } else {
            card_fill()
        },
    );
    let a = |x: f32, y: f32| r.min + egui::vec2(x, y);
    let line = |v: &[(f32, f32)]| {
        p.add(egui::Shape::line(
            v.iter().map(|(x, y)| a(*x, *y)).collect(),
            Stroke::new(1.5, color),
        ));
    };
    match icon {
        Icon::Select => line(&[
            (10., 6.),
            (10., 23.),
            (15., 18.),
            (21., 23.),
            (23., 21.),
            (18., 16.),
            (25., 14.),
            (10., 6.),
        ]),
        Icon::Wire => {
            line(&[(7., 9.), (15., 9.), (15., 21.), (25., 21.)]);
            p.circle_filled(a(7., 9.), 2., color);
            p.circle_filled(a(25., 21.), 2., color);
        }
        Icon::Pan => {
            line(&[(6., 15.), (26., 15.)]);
            line(&[(16., 5.), (16., 25.)]);
            line(&[(6., 15.), (10., 11.)]);
            line(&[(26., 15.), (22., 19.)]);
        }
        Icon::Fit => {
            for v in [
                &[(7., 12.), (7., 7.), (12., 7.)][..],
                &[(20., 7.), (25., 7.), (25., 12.)][..],
                &[(25., 18.), (25., 23.), (20., 23.)][..],
                &[(12., 23.), (7., 23.), (7., 18.)][..],
            ] {
                line(v);
            }
        }
        Icon::Play => {
            p.add(egui::Shape::convex_polygon(
                vec![a(11., 6.), a(25., 15.), a(11., 24.)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Stop => {
            p.rect_filled(Rect::from_min_max(a(9., 8.), a(24., 23.)), 2., color);
        }
        Icon::Save => {
            line(&[
                (7., 6.),
                (23., 6.),
                (26., 9.),
                (26., 24.),
                (7., 24.),
                (7., 6.),
            ]);
            line(&[(12., 6.), (12., 12.), (22., 12.), (22., 6.)]);
            line(&[(12., 24.), (12., 17.), (22., 17.), (22., 24.)]);
        }
        Icon::Open => line(&[
            (5., 23.),
            (5., 9.),
            (14., 9.),
            (16., 12.),
            (26., 12.),
            (23., 23.),
            (5., 23.),
            (10., 15.),
            (27., 15.),
        ]),
        Icon::Undo | Icon::Redo => {
            let v = if matches!(icon, Icon::Undo) {
                vec![
                    (8., 13.),
                    (14., 7.),
                    (14., 12.),
                    (23., 12.),
                    (25., 17.),
                    (21., 23.),
                ]
            } else {
                vec![
                    (24., 13.),
                    (18., 7.),
                    (18., 12.),
                    (9., 12.),
                    (7., 17.),
                    (11., 23.),
                ]
            };
            line(&v);
        }
        Icon::Network | Icon::Chip | Icon::Instrument | Icon::Python | Icon::Test | Icon::Wave => {
            let kind = match icon {
                Icon::Network => Kind::PnaX,
                Icon::Chip => Kind::Dut,
                Icon::Instrument => Kind::Generator,
                Icon::Python => Kind::Python,
                Icon::Test => Kind::Limit,
                _ => Kind::Awg,
            };
            symbol(p, r.shrink(3.), kind);
        }
        Icon::Settings => {
            p.circle_stroke(a(16., 15.), 7., Stroke::new(1.5, color));
            p.circle_stroke(a(16., 15.), 2., Stroke::new(1.5, color));
            for (x, y) in [(16., 5.), (16., 25.), (6., 15.), (26., 15.)] {
                p.circle_filled(a(x, y), 2., color);
            }
        }
        Icon::Export => {
            line(&[(8., 20.), (8., 25.), (25., 25.), (25., 20.)]);
            line(&[(16., 20.), (16., 5.), (11., 10.)]);
            line(&[(16., 5.), (21., 10.)]);
        }
        Icon::Grid => {
            for x in [9., 16., 23.] {
                for y in [8., 15., 22.] {
                    p.circle_filled(a(x, y), 1.8, color);
                }
            }
        }
        Icon::Clone => {
            p.rect_stroke(
                Rect::from_min_max(a(7., 6.), a(20., 19.)),
                2.,
                Stroke::new(1.5, color),
                StrokeKind::Inside,
            );
            p.rect_stroke(
                Rect::from_min_max(a(12., 11.), a(25., 24.)),
                2.,
                Stroke::new(1.5, color),
                StrokeKind::Inside,
            );
        }
    }
    response.on_hover_text(tooltip)
}
