use eframe::egui::{self, Color32, FontFamily, FontId, Stroke, TextStyle};
use std::cell::Cell;
thread_local! {static PALETTE:Cell<(bool,bool,f32)>=const {Cell::new((false,false,1.))};}
fn color(dark: [u8; 3], light: [u8; 3]) -> Color32 {
    let c = if PALETTE.get().0 { light } else { dark };
    Color32::from_rgb(c[0], c[1], c[2])
}
pub fn bg() -> Color32 {
    color([12, 18, 27], [241, 245, 250])
}
pub fn panel() -> Color32 {
    color([19, 27, 39], [255, 255, 255])
}
pub fn card_fill() -> Color32 {
    color([25, 36, 50], [234, 240, 247])
}
pub fn border() -> Color32 {
    if PALETTE.get().1 {
        color([185, 198, 211], [69, 84, 101])
    } else {
        color([45, 60, 77], [147, 167, 189])
    }
}
pub fn text_color() -> Color32 {
    color([226, 234, 244], [21, 35, 53])
}
pub fn muted() -> Color32 {
    if PALETTE.get().1 {
        text_color()
    } else {
        color([141, 158, 179], [72, 88, 107])
    }
}
pub fn teal() -> Color32 {
    color([72, 221, 196], [0, 104, 89])
}
pub fn blue() -> Color32 {
    color([94, 162, 255], [20, 79, 168])
}
pub fn purple() -> Color32 {
    color([185, 144, 255], [116, 43, 155])
}
pub fn gold() -> Color32 {
    color([250, 193, 104], [133, 79, 0])
}
pub fn red() -> Color32 {
    color([250, 118, 133], [176, 34, 51])
}
pub fn scale() -> f32 {
    PALETTE.get().2
}
pub fn font(size: f32) -> FontId {
    FontId::proportional(size * scale())
}
pub fn kind(kind: rf_core::Kind) -> Color32 {
    use rf_core::Kind::*;
    match kind {
        Dsp(_) => purple(),
        Generator | Awg | IqModulator => teal(),
        Dut | VariableResistor => gold(),
        Analyzer | Pna | PnaX | UsbVna | Adc | Dac => blue(),
        DcSupplyE3631A | DcSupplyE36313A => red(),
        Python => purple(),
        Peak | PowerSensor | PowerMeter => teal(),
        Limit | NoiseFigureMeter => gold(),
        Thermometer | Thermostream => red(),
    }
}
pub fn port(p: rf_core::Port) -> Color32 {
    use rf_core::Port::*;
    match p {
        ComplexIq | Bits | Spectrum | Quantity => purple(),
        Signal | DutModel => gold(),
        Trace | SParameters => blue(),
        Scalar | Analog => teal(),
        Digital => purple(),
        Temperature => red(),
        Resistance | NoiseFigure => gold(),
    }
}
pub fn setup(ctx: &egui::Context) {
    apply(ctx, false, false, 1.);
}
pub fn apply(ctx: &egui::Context, light: bool, contrast: bool, scale: f32) {
    PALETTE.set((light, contrast, scale));
    let theme = if light {
        egui::Theme::Light
    } else {
        egui::Theme::Dark
    };
    ctx.set_theme(theme);
    let mut v = if light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    v.override_text_color = Some(text_color());
    v.panel_fill = panel();
    v.window_fill = panel();
    v.extreme_bg_color = bg();
    v.faint_bg_color = card_fill();
    v.selection.bg_fill = teal().gamma_multiply(0.2);
    v.selection.stroke = Stroke::new(1.5, teal());
    v.widgets.noninteractive.bg_fill = card_fill();
    v.widgets.noninteractive.bg_stroke = Stroke::new(1., border());
    v.widgets.inactive.bg_fill = card_fill();
    v.widgets.inactive.weak_bg_fill = card_fill();
    v.widgets.inactive.bg_stroke = Stroke::new(1., border());
    v.widgets.hovered.bg_fill = teal().gamma_multiply(0.15);
    v.widgets.hovered.weak_bg_fill = card_fill();
    v.widgets.hovered.bg_stroke = Stroke::new(2., teal());
    v.widgets.active.bg_fill = teal().gamma_multiply(0.25);
    ctx.set_visuals_of(theme, v);
    ctx.style_mut_of(theme, |s| {
        s.spacing.item_spacing = egui::vec2(9., 8.);
        s.spacing.button_padding = egui::vec2(10., 7.);
        s.spacing.interact_size.y = 32. * scale;
        for (style, size, family) in [
            (TextStyle::Body, 14., FontFamily::Proportional),
            (TextStyle::Button, 14., FontFamily::Proportional),
            (TextStyle::Small, 11., FontFamily::Proportional),
            (TextStyle::Heading, 23., FontFamily::Proportional),
            (TextStyle::Monospace, 13., FontFamily::Monospace),
        ] {
            s.text_styles
                .insert(style, FontId::new(size * scale, family));
        }
    });
}
pub fn caption(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(crate::i18n::t(text))
            .size(11. * scale())
            .color(muted())
            .strong(),
    );
}
pub fn badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(
        egui::RichText::new(crate::i18n::t(text))
            .size(12. * scale())
            .color(color)
            .strong(),
    );
}
pub fn button(ui: &mut egui::Ui, text: &str, primary: bool) -> egui::Response {
    let mut b = egui::Button::new(egui::RichText::new(crate::i18n::t(text)).strong());
    if primary {
        b = egui::Button::new(
            egui::RichText::new(crate::i18n::t(text))
                .color(if PALETTE.get().0 {
                    Color32::WHITE
                } else {
                    bg()
                })
                .strong(),
        )
        .fill(teal());
    }
    ui.add(b)
}
