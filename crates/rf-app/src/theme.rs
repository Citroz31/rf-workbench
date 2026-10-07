use eframe::egui::{self, Color32, FontFamily, FontId, Stroke, TextStyle};
pub const BG: Color32 = Color32::from_rgb(12, 18, 27);
pub const PANEL: Color32 = Color32::from_rgb(19, 27, 39);
pub const CARD: Color32 = Color32::from_rgb(25, 36, 50);
pub const BORDER: Color32 = Color32::from_rgb(45, 60, 77);
pub const TEXT: Color32 = Color32::from_rgb(226, 234, 244);
pub const MUTED: Color32 = Color32::from_rgb(141, 158, 179);
pub const TEAL: Color32 = Color32::from_rgb(72, 221, 196);
pub const BLUE: Color32 = Color32::from_rgb(94, 162, 255);
pub const PURPLE: Color32 = Color32::from_rgb(185, 144, 255);
pub const GOLD: Color32 = Color32::from_rgb(250, 193, 104);
pub const RED: Color32 = Color32::from_rgb(250, 118, 133);

pub fn kind(kind: rf_core::Kind) -> Color32 {
    use rf_core::Kind::*;
    match kind {
        Generator | Awg | IqModulator => TEAL,
        Dut | VariableResistor => GOLD,
        Analyzer | Pna | PnaX | Adc | Dac => BLUE,
        Python => PURPLE,
        Peak | PowerSensor | PowerMeter => TEAL,
        Limit | NoiseFigureMeter => GOLD,
        Thermometer | Thermostream => RED,
    }
}
pub fn port(p: rf_core::Port) -> Color32 {
    match p {
        rf_core::Port::Signal | rf_core::Port::DutModel => GOLD,
        rf_core::Port::Trace | rf_core::Port::SParameters => BLUE,
        rf_core::Port::Scalar | rf_core::Port::Analog => TEAL,
        rf_core::Port::Digital => PURPLE,
        rf_core::Port::Temperature => RED,
        rf_core::Port::Resistance | rf_core::Port::NoiseFigure => GOLD,
    }
}
pub fn setup(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    let mut v = egui::Visuals::dark();
    v.override_text_color = Some(TEXT);
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.extreme_bg_color = BG;
    v.faint_bg_color = CARD;
    v.selection.bg_fill = Color32::from_rgb(30, 91, 88);
    v.selection.stroke = Stroke::new(1., TEAL);
    v.widgets.noninteractive.bg_fill = CARD;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1., BORDER);
    v.widgets.inactive.bg_fill = CARD;
    v.widgets.inactive.weak_bg_fill = CARD;
    v.widgets.inactive.bg_stroke = Stroke::new(1., BORDER);
    v.widgets.hovered.bg_fill = Color32::from_rgb(37, 55, 73);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(37, 55, 73);
    v.widgets.hovered.bg_stroke = Stroke::new(1., TEAL);
    v.widgets.active.bg_fill = Color32::from_rgb(32, 83, 79);
    ctx.set_visuals_of(egui::Theme::Dark, v);
    ctx.style_mut_of(egui::Theme::Dark, |s| {
        s.spacing.item_spacing = egui::vec2(10., 9.);
        s.spacing.button_padding = egui::vec2(12., 7.);
        s.text_styles
            .insert(TextStyle::Body, FontId::new(14., FontFamily::Proportional));
        s.text_styles.insert(
            TextStyle::Button,
            FontId::new(14., FontFamily::Proportional),
        );
        s.text_styles
            .insert(TextStyle::Small, FontId::new(11., FontFamily::Proportional));
        s.text_styles.insert(
            TextStyle::Heading,
            FontId::new(23., FontFamily::Proportional),
        );
        s.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(13., FontFamily::Monospace),
        );
    });
}
pub fn caption(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(11.).color(MUTED).strong());
}
pub fn badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(egui::RichText::new(text).size(12.).color(color).strong());
}
pub fn button(ui: &mut egui::Ui, text: &str, primary: bool) -> egui::Response {
    let mut b = egui::Button::new(egui::RichText::new(text).strong());
    if primary {
        b = egui::Button::new(egui::RichText::new(text).color(BG).strong()).fill(TEAL);
    }
    ui.add(b)
}
