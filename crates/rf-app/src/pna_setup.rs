//! Instrument / Response / Stimulus navigation and a shared, editable Quick Start.
use eframe::egui::{self, Align2, Rect, Sense, Stroke, StrokeKind};
use rf_core::{Config, Kind, pna_plan::{Arrangement, DisplayTrace, DisplayWindow, Format, Plan}};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    QuickStart,
    Trace,
    Channel,
    Window,
    Display,
    Frequency,
    Power,
    Sweep,
    AvgBw,
    Trigger,
    Calibration,
    Paths,
    Connection,
    Options,
    Console,
}
impl Page {
    pub fn label(self) -> &'static str {
        match self {
            Self::QuickStart => "Quick Start",
            Self::Trace => "Trace · Meas / Format / Scale",
            Self::Channel => "Channel · canal et ports",
            Self::Window => "Window · affectation des traces",
            Self::Display => "Display · écran instrument",
            Self::Frequency => "Freq · fréquences",
            Self::Power => "Power · source RF",
            Self::Sweep => "Sweep · balayage",
            Self::AvgBw => "Avg BW · IF et moyennage",
            Self::Trigger => "Trigger · acquisition",
            Self::Calibration => "Cal · calibration et fixtures",
            Self::Paths => "Path Configurator · RF / IF",
            Self::Connection => "Connexion · VISA / SCPI",
            Self::Options => "Meas Class · applications détectées",
            Self::Console => "Console SCPI",
        }
    }
    pub fn tab(self) -> u8 {
        match self {
            Self::Connection => 0,
            Self::Options => 2,
            Self::Console => 3,
            Self::Paths => 4,
            _ => 1,
        }
    }
}
pub fn navigation(ui: &mut egui::Ui, page: &mut Page, kind: Kind) {
    ui.horizontal_wrapped(|ui| {
        for (menu, items) in [
            ("Instrument", &[Page::Trace, Page::Channel, Page::Window, Page::Display, Page::QuickStart, Page::Options][..]),
            ("Response", &[Page::Trace, Page::AvgBw, Page::Calibration][..]),
            ("Stimulus", &[Page::Frequency, Page::Power, Page::Sweep, Page::Trigger][..]),
            ("Utility", &[Page::Connection, Page::Paths, Page::Console][..]),
        ] {
            ui.menu_button(menu, |ui| {
                for item in items {
                    if *item == Page::Paths && kind != Kind::PnaX { continue; }
                    if ui.selectable_label(*page == *item, item.label()).clicked() {
                        *page = *item;
                        ui.close();
                    }
                }
            });
        }
        if ui.button("Quick Start…").clicked() { *page = Page::QuickStart; }
    });
    ui.separator();
    ui.heading(page.label());
}

/// Reassigns the selected measurements without losing their format or vertical scale.
pub fn distribute(plan: &mut Plan, count: usize) {
    let old: Vec<_> = plan.windows.iter().flat_map(|w| w.traces.clone()).collect();
    let count = count.clamp(plan.parameters.len().div_ceil(16).max(1), plan.parameters.len().max(1));
    plan.windows = (0..count).map(|_| DisplayWindow { traces: Vec::new(), active_trace: 0 }).collect();
    for (i, p) in plan.parameters.iter().enumerate() {
        let trace = old.iter().find(|t| t.parameter == *p).cloned().unwrap_or_else(|| DisplayTrace {
            parameter: p.clone(),
            format: if p.starts_with('S') { Format::Magnitude } else { Format::Raw },
            ..Default::default()
        });
        plan.windows[i % count].traces.push(trace);
    }
    plan.arrangement = match count {
        1 => Arrangement::Single,
        2 => Arrangement::Two,
        3..=4 => Arrangement::Quad,
        5..=6 => Arrangement::Six,
        _ => Arrangement::Eight,
    };
}
fn parameter_sync(c: &mut Config) {
    if let Some(plan) = &c.instrument.pna.plan
        && !plan.parameters.contains(&c.s_parameter)
        && let Some(p) = plan.parameters.iter().find(|p| p.starts_with('S')) {
        c.s_parameter = p.clone();
    }
    c.instrument.measurement.clear();
    c.instrument.pna.all_s_parameters = false;
}
pub fn quick_start(ui: &mut egui::Ui, c: &mut Config) {
    ui.small(format!("Canal {} · {} ports · un plan pour la simulation et l'acquisition", c.instrument.channel, c.instrument.port_count));
    ui.horizontal_wrapped(|ui| {
        if ui.button("S11 / S12 / S21 / S22").clicked() {
            c.instrument.pna.power_sweep = false;
            c.instrument.pna.plan = Some(Plan::s_parameters(c.instrument.port_count));
            parameter_sync(c);
        }
        if ui.button("Matrice S complète").clicked() {
            c.instrument.pna.power_sweep = false;
            let mut plan = Plan::s_parameters(c.instrument.port_count);
            plan.parameters = (1..=c.instrument.port_count).flat_map(|i| (1..=c.instrument.port_count).map(move |j| format!("S{i}{j}"))).collect();
            let count = plan.parameters.len().min(8);
            distribute(&mut plan, count);
            c.instrument.pna.plan = Some(plan);
            parameter_sync(c);
        }
        if ui.button("Puissance · gain / R / récepteur").clicked() {
            let source = c.instrument.pna.source_port;
            c.instrument.pna.power_sweep = true;
            c.instrument.pna.plan = Some(Plan::power(source, if source == 1 { 2.min(c.instrument.port_count) } else { 1 }));
            parameter_sync(c);
        }
    });
    ui.separator();
    ui.strong("1 · Mesures à acquérir");
    let choices = crate::vna_keys::parameters(c);
    let plan = c.instrument.pna.plan.get_or_insert_with(|| Plan::s_parameters(c.instrument.port_count));
    let mut edited = false;
    ui.horizontal_wrapped(|ui| {
        for p in choices {
            let mut selected = plan.parameters.contains(&p);
            if ui.add_enabled(selected || plan.parameters.len() < 32, egui::Checkbox::new(&mut selected, &p)).changed() {
                if selected { plan.parameters.push(p); } else { plan.parameters.retain(|v| *v != p); }
                edited = true;
            }
        }
    });
    if edited {
        let count = plan.windows.len().max(1);
        distribute(plan, count);
    }
    ui.separator();
    ui.strong("2 · Nombre de fenêtres");
    let mut count = plan.windows.len().max(1);
    let before = count;
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("quick-start-windows").selected_text(format!("{count} fenêtre(s)"))
            .show_ui(ui, |ui| { for n in plan.parameters.len().div_ceil(16).max(1)..=plan.parameters.len().clamp(1, 8) {
                ui.selectable_value(&mut count, n, format!("{n} fenêtre(s)"));
            }});
        if ui.button("Une trace par fenêtre").clicked() { count = plan.parameters.len().clamp(1, 8); }
        if ui.button("Tout superposer").clicked() { count = 1; }
    });
    if count != before { distribute(plan, count); }
    ui.small("Chaque fenêtre contient au moins une trace ; jusqu'à huit fenêtres dans ce Quick Start.");
    assignments(ui, plan);
    preview(ui, plan);
    if let Err(e) = plan.validate(c.instrument.port_count, c.instrument.pna.power_sweep.then_some(c.instrument.pna.source_port)) {
        ui.colored_label(crate::theme::red(), e);
    }
    if edited { parameter_sync(c); }
}
pub fn assignments(ui: &mut egui::Ui, plan: &mut Plan) {
    ui.strong("3 · Affectation des traces");
    let mut movement = None;
    for (wi, window) in plan.windows.iter().enumerate() {
        for (ti, trace) in window.traces.iter().enumerate() {
            ui.horizontal_wrapped(|ui| {
                ui.label(&trace.parameter);
                let mut destination = wi;
                egui::ComboBox::from_id_salt(("trace-window", wi, ti)).selected_text(format!("Window {}", wi + 1))
                    .show_ui(ui, |ui| { for n in 0..plan.windows.len() {
                        ui.add_enabled_ui(n == wi || window.traces.len() > 1, |ui| {
                            ui.selectable_value(&mut destination, n, format!("Window {}", n + 1));
                        });
                    }});
                if destination != wi { movement = Some((wi, ti, destination)); }
            });
        }
    }
    if let Some((from, trace, to)) = movement {
        let trace = plan.windows[from].traces.remove(trace);
        plan.windows[from].active_trace = plan.windows[from].active_trace.min(plan.windows[from].traces.len() - 1);
        plan.windows[to].traces.push(trace);
    }
}
fn preview(ui: &mut egui::Ui, plan: &Plan) {
    let (columns, rows) = plan.arrangement.grid();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 150.), Sense::hover());
    for (i, w) in plan.windows.iter().enumerate() {
        let cell = Rect::from_min_size(rect.min + egui::vec2((i % columns) as f32 * rect.width() / columns as f32, (i / columns) as f32 * rect.height() / rows as f32), egui::vec2(rect.width() / columns as f32, rect.height() / rows as f32)).shrink(3.);
        let p = ui.painter_at(cell);
        p.rect_filled(cell, 3., crate::theme::bg());
        p.rect_stroke(cell, 3., Stroke::new(1., crate::theme::border()), StrokeKind::Inside);
        p.text(cell.center(), Align2::CENTER_CENTER, format!("Window {}\n{}", i + 1, w.traces.iter().map(|t| t.parameter.as_str()).collect::<Vec<_>>().join(" + ")), crate::theme::font(11.), crate::theme::gold());
    }
}
pub fn settings(ui: &mut egui::Ui, page: Page, c: &mut Config, kind: Kind) {
    match page {
        Page::QuickStart => quick_start(ui, c),
        Page::Trace => crate::pna_controls::trace_editor(ui, c),
        Page::Window => {
            let plan = c.instrument.pna.plan.get_or_insert_with(|| Plan::s_parameters(c.instrument.port_count));
            assignments(ui, plan);
            preview(ui, plan);
            ui.label("Revenir au Quick Start pour changer le nombre de fenêtres.");
        }
        Page::Channel => {
            ui.add(egui::DragValue::new(&mut c.instrument.channel).range(1..=1000).prefix("Channel "));
            ui.label("Ce bloc configure un canal. Les fréquences, IFBW et puissance appartiennent à ce canal ; les fenêtres contiennent ses traces.");
            egui::ComboBox::from_id_salt("vna-ports").selected_text(format!("{} ports RF", c.instrument.port_count)).show_ui(ui, |ui| {
                for n in 1..=kind.max_rf_ports().unwrap_or(4) { ui.selectable_value(&mut c.instrument.port_count, n, format!("{n} ports RF")); }
            });
            ui.small("Le schéma est mis à jour immédiatement. Réduire le nombre de ports retire leurs câbles.");
            egui::ComboBox::from_id_salt("pna-precision").selected_text(format!("REAL{} · SDATA binaire", c.instrument.precision)).show_ui(ui, |ui| {
                ui.selectable_value(&mut c.instrument.precision, 32, "REAL32 · compact");
                ui.selectable_value(&mut c.instrument.precision, 64, "REAL64");
            });
        }
        Page::Display => {
            let plan = c.instrument.pna.plan.get_or_insert_with(|| Plan::s_parameters(c.instrument.port_count));
            ui.checkbox(&mut plan.synchronize_display, "Synchroniser fenêtres, traces, formats et échelles sur le VNA");
            ui.add(egui::DragValue::new(&mut plan.first_window).range(1..=1024).prefix("Première Window sur l'appareil : "));
            preview(ui, plan);
            ui.small("Les fenêtres désignées sont remplacées lors de Configurer. Les autres fenêtres et les calibrations de l'appareil sont conservées.");
        }
        Page::Frequency => {
            crate::frequency_edit::frequency(ui, "Start", &mut c.start_hz, &mut c.frequency_unit);
            crate::frequency_edit::frequency(ui, "Stop", &mut c.stop_hz, &mut c.frequency_unit);
            crate::frequency_edit::sweep_step(ui, c);
            if c.instrument.pna.power_sweep { crate::frequency_edit::frequency(ui, "CW", &mut c.frequency_hz, &mut c.frequency_unit); }
        }
        Page::Power => {
            ui.add(egui::DragValue::new(&mut c.power_dbm).speed(0.1).prefix("RF level : ").suffix(" dBm"));
            let previous = c.instrument.pna.source_port;
            ui.add(egui::DragValue::new(&mut c.instrument.pna.source_port).range(1..=c.instrument.port_count).prefix("Source P"));
            if c.instrument.pna.power_sweep {
                ui.add(egui::DragValue::new(&mut c.instrument.pna.power_start_dbm).speed(0.1).prefix("Start : ").suffix(" dBm"));
                ui.add(egui::DragValue::new(&mut c.instrument.pna.power_stop_dbm).speed(0.1).prefix("Stop : ").suffix(" dBm"));
                crate::frequency_edit::frequency(ui, "CW", &mut c.frequency_hz, &mut c.frequency_unit);
                if previous != c.instrument.pna.source_port {
                    let source = c.instrument.pna.source_port;
                    c.instrument.pna.plan = Some(Plan::power(source, if source == 1 { 2.min(c.instrument.port_count) } else { 1 }));
                    parameter_sync(c);
                }
            }
        }
        Page::Sweep => {
            let previous = c.instrument.pna.power_sweep;
            ui.checkbox(&mut c.instrument.pna.power_sweep, "Power Sweep · fréquence CW");
            if previous != c.instrument.pna.power_sweep {
                let source = c.instrument.pna.source_port;
                c.instrument.pna.plan = Some(if c.instrument.pna.power_sweep { Plan::power(source, if source == 1 { 2.min(c.instrument.port_count) } else { 1 }) } else { Plan::s_parameters(c.instrument.port_count) });
                parameter_sync(c);
            }
            ui.add(egui::DragValue::new(&mut c.points).range(2..=rf_core::MAX_POINTS).prefix("Points : "));
            ui.checkbox(&mut c.instrument.configure_sweep, "Configurer le canal avant Exécuter");
            ui.label("Balayage fréquentiel : réglages dans Freq. Balayage en puissance : réglages dans Power.");
        }
        Page::AvgBw => {
            crate::frequency_edit::frequency(ui, "IF bandwidth", &mut c.instrument.if_bandwidth_hz, &mut c.if_unit);
            ui.checkbox(&mut c.instrument.averaging, "Moyennage actif");
            ui.add(egui::DragValue::new(&mut c.instrument.averages).range(1..=65536).suffix(" acquisitions"));
        }
        Page::Trigger => {
            ui.checkbox(&mut c.instrument.trigger, "Single Sweep · déclencher et attendre *OPC?");
            ui.label("Décoché : lire les dernières données du canal. Aucun sweep n'est lancé lors d'une simple modification du brouillon.");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_quick_start_buttons_change_the_shared_plan_without_changing_sweep_values() {
        let ctx = egui::Context::default();
        let mut c = Config::default();
        let sweep = (c.start_hz, c.stop_hz, c.instrument.if_bandwidth_hz, c.power_dbm);
        let mut render = |events| {
            let frame = ctx.run(egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 900.))), events, ..Default::default()
            }, |ctx| { egui::CentralPanel::default().show(ctx, |ui| quick_start(ui, &mut c)); });
            (frame, c.clone())
        };
        let text_position = |frame: &egui::FullOutput, value: &str| {
            frame.shapes.iter().find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == value => Some(t.pos + t.galley.rect.center().to_vec2()),
                _ => None
            }).unwrap_or_else(|| panic!("Missing button {value}"))
        };
        for (button, expected) in [("Tout superposer", 1), ("Une trace par fenêtre", 4)] {
            let (frame, _) = render(vec![]);
            let pos = text_position(&frame, button);
            render(vec![egui::Event::PointerMoved(pos)]);
            for pressed in [true, false] {
                render(vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }]);
            }
            let (_, draft) = render(vec![]);
            let plan = draft.instrument.pna.plan.as_ref().unwrap();
            assert_eq!(plan.windows.len(), expected);
            assert_eq!(plan.parameters, ["S11", "S12", "S21", "S22"]);
            assert_eq!((draft.start_hz, draft.stop_hz, draft.instrument.if_bandwidth_hz, draft.power_dbm), sweep);
            plan.validate(draft.instrument.port_count, None).unwrap();
        }
    }
    #[test]
    fn one_port_power_preset_has_no_duplicate_measurements() {
        let plan = Plan::power(1, 1);
        plan.validate(1, Some(1)).unwrap();
        assert_eq!(plan.parameters, ["S11", "R1_1", "A_1"]);
    }
    #[test]
    fn quick_start_distribution_preserves_scales_and_roundtrips_six_windows() {
        let mut p = Plan::s_parameters(4);
        p.parameters = (1..=4).flat_map(|i| (1..=4).map(move |j| format!("S{i}{j}"))).collect();
        p.windows[0].traces[0].automatic = false;
        p.windows[0].traces[0].per_division = 2.;
        distribute(&mut p, 6);
        assert_eq!(p.windows.len(), 6);
        assert_eq!(p.arrangement.grid(), (3, 2));
        assert_eq!(p.windows[0].traces[0].per_division, 2.);
        p.validate(4, None).unwrap();
        let restored: Plan = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(restored, p);
        distribute(&mut p, 1);
        assert_eq!(p.windows[0].traces.len(), 16);
        p.validate(4, None).unwrap();
    }
}
