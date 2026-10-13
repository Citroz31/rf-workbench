//! Virtual front panel. Edits are staged and applied to the shared project plan.
use crate::{
    frequency_edit,
    results::{Format, Panel, Trace, Window},
    theme::*,
};
use eframe::egui;
use rf_core::{Config, Graph, network::Curve};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Key {
    Trace,
    Channel,
    Display,
    Setup,
    Meas,
    Format,
    Scale,
    AvgBw,
    Cal,
    Marker,
    Search,
    #[default]
    Freq,
    Power,
    Sweep,
    Trigger,
    SaveRecall,
    System,
    Undo,
    Help,
    Preset,
}
impl Key {
    pub fn label(self) -> &'static str {
        match self {
            Self::Trace => "Trace",
            Self::Channel => "Channel",
            Self::Display => "Display",
            Self::Setup => "Setup",
            Self::Meas => "Meas",
            Self::Format => "Format",
            Self::Scale => "Scale",
            Self::AvgBw => "Avg / BW",
            Self::Cal => "Cal",
            Self::Marker => "Marker",
            Self::Search => "Search",
            Self::Freq => "Freq",
            Self::Power => "Power",
            Self::Sweep => "Sweep",
            Self::Trigger => "Trigger",
            Self::SaveRecall => "Save / Recall",
            Self::System => "System",
            Self::Undo => "Undo",
            Self::Help => "Help",
            Self::Preset => "Preset",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Start,
    Stop,
    Center,
    Span,
    Step,
    Cw,
}
#[derive(Clone, Copy, Debug)]
pub enum Action {
    Configure(u64),
    Run,
    Stop,
    Instrument(u64),
    Fixture(u64),
    Save,
    Open,
    Undo,
    Help,
    Relayout(u64),
}
#[derive(Clone, Debug)]
pub struct Editor {
    key: Key,
    field: Option<Field>,
    window: u64,
    trace: usize,
    node: u64,
    config: Config,
    display: Trace,
    grid: (usize, usize),
    error: String,
    original_config: Config,
    original_trace: Trace,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    windows: Vec<Window>,
    grid: (usize, usize),
    configs: Vec<(u64, Config)>,
}
impl Snapshot {
    pub fn capture(panel: &Panel, windows: &[Window], graph: &Graph) -> Self {
        Self {
            windows: windows.to_vec(),
            grid: (panel.columns, panel.rows),
            configs: graph
                .nodes
                .iter()
                .filter(|n| n.kind.max_rf_ports().is_some())
                .map(|n| (n.id, n.config.clone()))
                .collect(),
        }
    }
    pub fn restore(self, panel: &mut Panel, windows: &mut Vec<Window>, graph: &mut Graph) {
        *windows = self.windows;
        (panel.columns, panel.rows) = self.grid;
        panel.page = 0;
        panel.editor = None;
        for (id, config) in self.configs {
            if let Some(n) = graph.nodes.iter_mut().find(|n| n.id == id) {
                n.config = config;
            }
        }
    }
}
pub fn parameters(config: &Config) -> Vec<String> {
    let ports = config.instrument.port_count;
    let source = config.instrument.pna.source_port;
    let mut out = Vec::new();
    for i in 1..=ports {
        for j in 1..=ports {
            if !config.instrument.pna.power_sweep || j == source {
                out.push(format!("S{i}{j}"));
            }
        }
    }
    for j in 1..=ports {
        if config.instrument.pna.power_sweep && j != source {
            continue;
        }
        for i in 1..=ports {
            out.push(format!("R{i}_{j}"));
            out.push(format!("{}_{j}", ["A", "B", "C", "D"][usize::from(i - 1)]));
        }
    }
    out
}
pub fn formats(ui: &mut egui::Ui, trace: &mut Trace) {
    for (format, label) in [
        (Format::Magnitude, "Module · dB / dBm"),
        (Format::Phase, "Phase · degrés"),
        (Format::Smith, "Smith · réflexion complexe"),
        (Format::Linear, "Données brutes"),
    ] {
        ui.add_enabled_ui(
            format != Format::Smith
                || matches!(trace.parameter.as_str(), "S11" | "S22" | "S33" | "S44"),
            |ui| {
                ui.selectable_value(&mut trace.format, format, label);
            },
        );
    }
}
pub fn measurement(ui: &mut egui::Ui, trace: &mut Trace, choices: &[String]) {
    egui::ComboBox::from_id_salt("measurement")
        .selected_text(&trace.parameter)
        .show_ui(ui, |ui| {
            for parameter in choices {
                ui.selectable_value(&mut trace.parameter, parameter.clone(), parameter);
            }
        });
    if trace.format == Format::Smith
        && !matches!(trace.parameter.as_str(), "S11" | "S22" | "S33" | "S44")
    {
        trace.format = Format::Magnitude;
    }
}
pub fn trace_menu(ui: &mut egui::Ui, window: &mut Window, index: usize, choices: &[String]) {
    window.active_trace = index.min(window.traces.len() - 1);
    let trace = &mut window.traces[window.active_trace];
    ui.strong(format!("Trace {} · {}", index + 1, trace.parameter));
    ui.label("Mesure");
    measurement(ui, trace, choices);
    ui.menu_button("Format", |ui| formats(ui, trace));
    ui.checkbox(&mut trace.visible, "Afficher la trace");
    if ui.button("Échelle automatique · cette trace").clicked() {
        trace.scale.automatic = true;
        ui.close();
    }
    ui.menu_button("Échelle manuelle…", |ui| scale(ui, &mut trace.scale));
    if window.traces.len() > 1 && ui.button("Retirer cette trace").clicked() {
        window.traces.remove(window.active_trace);
        window.active_trace = window.active_trace.min(window.traces.len() - 1);
        ui.close();
    }
    ui.separator();
    add_menu(ui, window, choices);
}
pub fn add_menu(ui: &mut egui::Ui, window: &mut Window, choices: &[String]) {
    ui.add_enabled_ui(window.traces.len() < 16, |ui| {
        ui.menu_button("Ajouter une trace dans cette fenêtre", |ui| {
            egui::ScrollArea::vertical()
                .max_height(280.)
                .show(ui, |ui| {
                    for parameter in choices {
                        if ui.button(parameter).clicked() {
                            let format = if parameter.starts_with('S') {
                                Format::Magnitude
                            } else {
                                Format::Linear
                            };
                            window.add_trace(
                                window.node,
                                parameter.clone(),
                                format,
                                window.corrected,
                            );
                            ui.close();
                        }
                    }
                });
        });
    });
}
fn scale(ui: &mut egui::Ui, scale: &mut crate::results::Scale) {
    ui.checkbox(&mut scale.automatic, "Échelle automatique");
    ui.add_enabled_ui(!scale.automatic, |ui| {
        ui.add(
            egui::DragValue::new(&mut scale.per_division)
                .range(1e-9..=1e12)
                .speed(0.1)
                .prefix("Échelle / division : "),
        );
        ui.add(
            egui::DragValue::new(&mut scale.reference)
                .range(-1e15..=1e15)
                .speed(0.1)
                .prefix("Référence : "),
        );
        ui.add(
            egui::DragValue::new(&mut scale.reference_division)
                .range(0. ..=10.)
                .speed(0.1)
                .prefix("Position depuis le haut : ")
                .suffix(" div"),
        );
    });
}
pub(crate) fn open(
    panel: &mut Panel,
    window: &Window,
    config: &Config,
    key: Key,
    field: Option<Field>,
) {
    panel.hotkey = key;
    panel.editor = Some(Editor {
        key,
        field,
        window: window.id,
        trace: window.active_trace,
        node: window.node,
        config: config.clone(),
        display: window.traces[window.active_trace].clone(),
        grid: (panel.columns, panel.rows),
        error: String::new(),
        original_config: config.clone(),
        original_trace: window.traces[window.active_trace].clone(),
    });
}
pub fn side(ui: &mut egui::Ui, panel: &mut Panel, windows: &mut [Window], graph: &Graph) {
    let Some(window) = windows.iter_mut().find(|w| w.id == panel.selected) else {
        return;
    };
    let Some(node) = graph.node(window.node) else {
        return;
    };
    let c = &node.config;
    let trace = &window.traces[window.active_trace];
    let area = ui.available_rect_before_wrap();
    let hard_width = if area.width() >= 230. { 116. } else { 62. };
    let soft = egui::Rect::from_min_max(
        area.min,
        egui::pos2(area.right() - hard_width - 8., area.bottom()),
    );
    let hard = egui::Rect::from_min_max(egui::pos2(soft.right() + 8., area.top()), area.max);
    crate::bounded_ui::region(ui, soft, "softkeys-region", |ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        ui.heading(panel.hotkey.label());
        ui.small(format!(
            "Ch {} · Tr {} · {}",
            c.instrument.channel,
            window.active_trace + 1,
            trace.parameter
        ));
        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .min_scrolled_width(0.)
            .min_scrolled_height(0.)
            .id_salt("vna-softkeys")
            .show(ui, |ui| {
                if panel.hotkey == Key::Freq {
                    let unit = c.frequency_unit;
                    for (label, value, field) in [
                        ("Start", c.start_hz, Field::Start),
                        ("Stop", c.stop_hz, Field::Stop),
                        ("Center", (c.start_hz + c.stop_hz) / 2., Field::Center),
                        ("Span", c.stop_hz - c.start_hz, Field::Span),
                        (
                            "Step",
                            (c.stop_hz - c.start_hz) / (c.points - 1) as f64,
                            Field::Step,
                        ),
                        ("CW", c.frequency_hz, Field::Cw),
                    ] {
                        let unit = if field == Field::Step {
                            c.step_unit
                        } else {
                            unit
                        };
                        if ui
                            .add_sized(
                                [ui.available_width(), 52.],
                                egui::Button::new(format!(
                                    "{label}\n{:.6} {}",
                                    value / unit.factor(),
                                    unit.label()
                                )),
                            )
                            .clicked()
                        {
                            open(panel, window, c, Key::Freq, Some(field));
                        }
                    }
                } else {
                    let description = match panel.hotkey {
                        Key::Scale => {
                            if trace.scale.automatic {
                                format!("{}\nÉchelle automatique", trace.parameter)
                            } else {
                                format!(
                                    "{}\n{:.3} / div\nRéf. {:.3}",
                                    trace.parameter,
                                    trace.scale.per_division,
                                    trace.scale.reference
                                )
                            }
                        }
                        Key::AvgBw => format!(
                            "IF BW {:.3} {}\n{} acquisitions",
                            c.instrument.if_bandwidth_hz / c.if_unit.factor(),
                            c.if_unit.label(),
                            c.instrument.averages
                        ),
                        Key::Power => format!(
                            "{:.2} dBm\nSource P{}",
                            c.power_dbm, c.instrument.pna.source_port
                        ),
                        Key::Display => format!("{} × {} fenêtres", panel.columns, panel.rows),
                        Key::Channel => format!(
                            "Canal {}\n{} ports",
                            c.instrument.channel, c.instrument.port_count
                        ),
                        Key::Sweep => format!(
                            "{} points\n{}",
                            c.points,
                            if c.instrument.pna.power_sweep {
                                "Puissance CW"
                            } else {
                                "Fréquence"
                            }
                        ),
                        _ => format!("{}\n{}", panel.hotkey.label(), trace.parameter),
                    };
                    if ui
                        .add_sized([ui.available_width(), 75.], egui::Button::new(description))
                        .clicked()
                    {
                        open(panel, window, c, panel.hotkey, None);
                    }
                }
                ui.add_space(8.);
                ui.small(
                    "Sélectionner une trace dans les graphes. Clic droit : modifier ou superposer.",
                );
                if ui.button("Masquer les touches").clicked() {
                    panel.keys_visible = false;
                }
            });
    });
    crate::bounded_ui::region(ui, hard, "hardkeys-region", |ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        ui.strong("Hotkeys");
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .min_scrolled_width(0.)
            .min_scrolled_height(0.)
            .id_salt("vna-hardkeys")
            .show(ui, |ui| {
                for (section, pairs) in [
                    (
                        "Instrument",
                        vec![(Key::Trace, Key::Channel), (Key::Display, Key::Setup)],
                    ),
                    (
                        "Response",
                        vec![
                            (Key::Meas, Key::Format),
                            (Key::Scale, Key::AvgBw),
                            (Key::Cal, Key::Marker),
                            (Key::Search, Key::Help),
                        ],
                    ),
                    (
                        "Stimulus",
                        vec![(Key::Freq, Key::Power), (Key::Sweep, Key::Trigger)],
                    ),
                    (
                        "Utility",
                        vec![(Key::SaveRecall, Key::System), (Key::Undo, Key::Preset)],
                    ),
                ] {
                    ui.small(section);
                    for (a, b) in pairs {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.;
                            for key in [a, b] {
                                if ui
                                    .add_sized(
                                        [55., 42.],
                                        egui::Button::new(
                                            egui::RichText::new(key.label()).size(11.),
                                        )
                                        .selected(panel.hotkey == key),
                                    )
                                    .clicked()
                                {
                                    open(panel, window, c, key, None);
                                }
                            }
                        });
                    }
                }
            });
    });
}
fn frequency_fields(ui: &mut egui::Ui, c: &mut Config, field: Option<Field>) {
    let mut center = (c.start_hz + c.stop_hz) / 2.;
    let mut span = c.stop_hz - c.start_hz;
    if field.is_none() || field == Some(Field::Start) {
        frequency_edit::frequency(ui, "Start", &mut c.start_hz, &mut c.frequency_unit);
    }
    if field.is_none() || field == Some(Field::Stop) {
        frequency_edit::frequency(ui, "Stop", &mut c.stop_hz, &mut c.frequency_unit);
    }
    if field == Some(Field::Center)
        && frequency_edit::frequency(ui, "Center", &mut center, &mut c.frequency_unit)
    {
        c.start_hz = center - span / 2.;
        c.stop_hz = center + span / 2.;
    }
    if field == Some(Field::Span)
        && frequency_edit::frequency(ui, "Span", &mut span, &mut c.frequency_unit)
    {
        c.start_hz = center - span / 2.;
        c.stop_hz = center + span / 2.;
    }
    if field.is_none() || field == Some(Field::Step) {
        frequency_edit::sweep_step(ui, c);
    }
    if field.is_none() || field == Some(Field::Cw) {
        frequency_edit::frequency(ui, "CW", &mut c.frequency_hz, &mut c.frequency_unit);
    }
}
pub fn peak(trace: &Trace, data: &[Curve], maximum: bool) -> Option<f64> {
    let curve = data.iter().rev().find(|c| {
        c.node == trace.node && c.name == trace.parameter && c.corrected == trace.corrected
    })?;
    let y = if trace.format == Format::Phase {
        curve.phase_deg.as_ref()?
    } else {
        &curve.y
    };
    let i = y
        .iter()
        .enumerate()
        .filter(|(_, v)| v.is_finite())
        .min_by(|(_, a), (_, b)| {
            if maximum {
                b.total_cmp(a)
            } else {
                a.total_cmp(b)
            }
        })?
        .0;
    curve.x.get(i).copied()
}
pub fn popup(
    ctx: &egui::Context,
    panel: &mut Panel,
    windows: &mut [Window],
    graph: &mut Graph,
    data: &[Curve],
    busy: bool,
) -> Vec<Action> {
    let Some(mut editor) = panel.editor.take() else {
        return Vec::new();
    };
    let mut actions = Vec::new();
    let mut opened = true;
    let mut apply = false;
    let mut cancel = false;
    egui::Window::new(format!("VNA · {}", editor.key.label())).id(egui::Id::new("vna-key-editor"))
        .open(&mut opened).collapsible(false).resizable(true).default_width(400.).show(ctx, |ui| {
        let e = &mut editor; let c = &mut e.config;
        ui.small(format!("Bloc {} · canal {} · graphique {} · trace {}", e.node, c.instrument.channel, e.window, e.trace+1));
        ui.separator();
        match e.key {
            Key::Freq => frequency_fields(ui, c, e.field),
            Key::Power => {
                ui.add(egui::DragValue::new(&mut c.power_dbm).speed(0.1).range(-160. ..=30.).prefix("Niveau RF : ").suffix(" dBm"));
                ui.add(egui::DragValue::new(&mut c.instrument.pna.power_start_dbm).speed(0.1).range(-160. ..=30.).prefix("Début sweep : ").suffix(" dBm"));
                ui.add(egui::DragValue::new(&mut c.instrument.pna.power_stop_dbm).speed(0.1).range(-160. ..=30.).prefix("Fin sweep : ").suffix(" dBm"));
                ui.label(format!("Port source P{} (modifier dans Setup)", c.instrument.pna.source_port));
            }
            Key::AvgBw => {
                frequency_edit::frequency(ui, "IF bandwidth", &mut c.instrument.if_bandwidth_hz, &mut c.if_unit);
                ui.checkbox(&mut c.instrument.averaging, "Moyennage actif");
                ui.add(egui::DragValue::new(&mut c.instrument.averages).range(1..=65536).suffix(" acquisitions"));
            }
            Key::Channel => { ui.add(egui::DragValue::new(&mut c.instrument.channel).range(1..=1000).prefix("Canal : ")); ui.label("Le canal du bloc s'applique à toutes ses fenêtres et traces."); }
            Key::Sweep => {
                ui.add(egui::DragValue::new(&mut c.points).range(2..=rf_core::MAX_POINTS).prefix("Points : "));
                let previous = c.instrument.pna.power_sweep;
                ui.checkbox(&mut c.instrument.pna.power_sweep, "Balayage de puissance à fréquence CW");
                if previous != c.instrument.pna.power_sweep {
                    c.instrument.pna.plan = Some(if c.instrument.pna.power_sweep { rf_core::pna_plan::Plan::power(c.instrument.pna.source_port, if c.instrument.pna.source_port == 1 {2.min(c.instrument.port_count)} else {1}) } else { rf_core::pna_plan::Plan::s_parameters(c.instrument.port_count) });
                }
                frequency_fields(ui,c,None);
                ui.label("Le changement de mode reconstruit les traces du panneau après validation.");
            }
            Key::Trace | Key::Meas => {
                ui.label("Paramètre de la trace sélectionnée"); measurement(ui, &mut e.display, &parameters(c));
                ui.checkbox(&mut e.display.visible, "Afficher cette trace");
                ui.label("Clic droit dans le graphique pour ajouter d'autres traces à la même fenêtre.");
            }
            Key::Format => formats(ui, &mut e.display),
            Key::Scale => {
                if e.display.scale.automatic
                    && let Some(curve)=data.iter().rev().find(|d|d.node==e.display.node && d.name==e.display.parameter && d.corrected==e.display.corrected) {
                        let values=if e.display.format==Format::Phase { curve.phase_deg.as_deref() } else { Some(curve.y.as_slice()) };
                        let bounds=e.display.scale.bounds(values);
                        e.display.scale.per_division=(bounds.high-bounds.low)/10.;
                        e.display.scale.reference=(bounds.high+bounds.low)/2.;
                        e.display.scale.reference_division=5.;
                }
                scale(ui, &mut e.display.scale);
            },
            Key::Display => {
                for (label,grid) in [("1 fenêtre",(1,1)),("2 fenêtres",(2,1)),("4 fenêtres",(2,2)),("6 fenêtres",(3,2)),("8 fenêtres",(4,2))] { ui.selectable_value(&mut e.grid,grid,label); }
            }
            Key::Marker | Key::Search => {
                let mut enabled = e.display.marker.is_some();
                if ui.checkbox(&mut enabled,"Marqueur M1 local").changed() {
                    let start=data.iter().rev().find(|d|d.node==e.display.node && d.name==e.display.parameter && d.corrected==e.display.corrected).and_then(|d|d.x.first()).copied().unwrap_or(if c.instrument.pna.power_sweep { c.instrument.pna.power_start_dbm } else { c.start_hz });
                    e.display.marker=enabled.then_some(start);
                }
                if let Some(x) = &mut e.display.marker {
                    if c.instrument.pna.power_sweep { ui.add(egui::DragValue::new(x).speed(0.1).range(-160. ..=30.).prefix("Position : ").suffix(" dBm")); }
                    else { frequency_edit::frequency(ui, "Position du marqueur", x, &mut c.frequency_unit); }
                }
                for (label,max) in [("Maximum de la trace",true),("Minimum de la trace",false)] {
                    if ui.add_enabled(peak(&e.display,data,max).is_some(),egui::Button::new(label)).clicked() { e.display.marker=peak(&e.display,data,max); }
                }
                ui.label("Recherche sur les données affichées. Le marqueur n'est pas envoyé au PNA physique.");
            }
            Key::Cal => {
                ui.label("CalSet existant sur l'instrument"); ui.text_edit_singleline(&mut c.instrument.pna.calset);
                ui.checkbox(&mut c.instrument.pna.apply_calset,"Appliquer ce CalSet");
                ui.checkbox(&mut c.instrument.pna.fixture.enabled,"Déembedding logiciel actif");
                if ui.button("Fixtures / AFR 2×Thru…").clicked() { actions.push(Action::Fixture(e.node)); cancel=true; }
                if ui.button("Catalogue CalSets / options instrument…").clicked() { actions.push(Action::Instrument(e.node)); cancel=true; }
            }
            Key::Trigger => {
                ui.checkbox(&mut c.instrument.configure_sweep,"Appliquer le plan avant l'acquisition");
                ui.label("Un seul balayage partagé par les traces. Run respecte le mode Simulation / Matériel de l'application.");
                if ui.add_enabled(!busy,egui::Button::new("Balayage / Run du banc")).clicked() { apply=true; actions.push(Action::Run); }
                if ui.add_enabled(busy,egui::Button::new("Stop")).clicked() { actions.push(Action::Stop); cancel=true; }
            }
            Key::Setup | Key::System => {
                ui.label(&c.resource); ui.label("Réglages avancés, connexions et chemins RF/IF du bloc actif.");
                if ui.button("Ouvrir les réglages du bloc…").clicked() { actions.push(Action::Instrument(e.node)); cancel=true; }
            }
            Key::SaveRecall => {
                ui.label("Sauvegarder ou charger un projet .rfbench indépendant.");
                for (label,action) in [("Enregistrer le projet…",Action::Save),("Charger un projet…",Action::Open)] { if ui.add_enabled(!busy,egui::Button::new(label)).clicked() { actions.push(action); cancel=true; } }
            }
            Key::Undo => { ui.label("Annuler le dernier changement du banc ou d'un réglage instrument."); if ui.button("Annuler").clicked() { actions.push(Action::Undo); cancel=true; } }
            Key::Help => { ui.label("Clic droit sur une étiquette de trace : mesure, format et ajout de traces. Les graduations suivent la trace active ; chaque trace conserve son échelle."); if ui.button("Aide détaillée du bloc").clicked() { actions.push(Action::Help); cancel=true; } }
            Key::Preset => { ui.label("Restaurer le plan paramètres S : S11, S12, S21, S22 en dB, un canal et quatre fenêtres (selon ports disponibles). Les connexions du banc sont conservées."); }
        }
        ui.separator();
        if !e.error.is_empty() { ui.colored_label(red(), &e.error); }
        ui.small("Appliquer modifie le projet. L'instrument est configuré par l'action explicite Configurer ou lors du prochain Run si l'application du plan est activée.");
        ui.horizontal(|ui| {
            if ui.button("Appliquer").clicked() { apply=true; }
            if ui.button("Fermer / annuler").clicked() { cancel=true; }
            if ui.add_enabled(!busy,egui::Button::new("Appliquer + configurer PNA")).clicked() { apply=true; actions.push(Action::Configure(e.node)); }
        });
    });
    if apply {
        let rebuild = editor.key == Key::Preset
            || (editor.key == Key::Sweep
                && graph.node(editor.node).is_some_and(|n| {
                    n.config.instrument.pna.power_sweep != editor.config.instrument.pna.power_sweep
                }));
        let result = apply_editor(&editor, panel, windows, graph);
        if let Err(error) = result {
            editor.error = error;
            actions.clear();
            panel.editor = Some(editor);
        } else if rebuild {
            actions.insert(0, Action::Relayout(editor.node));
        }
    } else if opened && !cancel {
        panel.editor = Some(editor);
    }
    actions
}
fn apply_editor(
    e: &Editor,
    panel: &mut Panel,
    windows: &mut [Window],
    graph: &mut Graph,
) -> Result<(), String> {
    let config = &e.config;
    config.instrument.validate()?;
    if !config.start_hz.is_finite()
        || !config.stop_hz.is_finite()
        || config.start_hz <= 0.
        || config.stop_hz <= config.start_hz
        || !config.frequency_hz.is_finite()
        || config.frequency_hz <= 0.
        || !(2..=rf_core::MAX_POINTS).contains(&config.points)
        || !config.power_dbm.is_finite()
        || !(-160. ..=30.).contains(&config.power_dbm)
    {
        return Err("Fréquences, puissance ou points invalides : Start > 0, Stop > Start.".into());
    }
    let window = windows
        .iter_mut()
        .find(|w| w.id == e.window)
        .ok_or("Graphique fermé")?;
    let trace = window.traces.get_mut(e.trace).ok_or("Trace retirée")?;
    let node = graph
        .nodes
        .iter_mut()
        .find(|n| n.id == e.node)
        .ok_or("Bloc retiré")?;
    // Refuse a stale dialog rather than redirecting its edits to a different trace.
    if *trace != e.original_trace || node.config != e.original_config {
        return Err("La trace ou le bloc a changé : rouvrir le réglage pour conserver les dernières modifications.".into());
    }
    node.config = config.clone();
    *trace = e.display.clone();
    if e.key == Key::Display {
        (panel.columns, panel.rows) = e.grid;
        panel.page = 0;
    }
    if e.key == Key::Preset {
        node.config.instrument.pna.power_sweep = false;
        node.config.instrument.pna.all_s_parameters = false;
        node.config.instrument.pna.plan = Some(rf_core::pna_plan::Plan::s_parameters(
            node.config.instrument.port_count,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bench() -> (Graph, Panel, Vec<Window>) {
        let mut graph = Graph::default();
        let id = graph.add(rf_core::Kind::PnaX, [0., 0.]);
        let mut window = Window::new(1, id, "S11".into(), Format::Magnitude, false);
        window.ensure_traces();
        window.add_trace(id, "S22".into(), Format::Magnitude, false);
        window.active_trace = 0;
        (
            graph,
            Panel {
                selected: 1,
                ..Default::default()
            },
            vec![window],
        )
    }
    #[test]
    fn edits_preserve_other_scales_reject_bad_ranges_and_detect_stale_dialogs() {
        let (mut graph, mut panel, mut windows) = bench();
        let id = windows[0].node;
        windows[0].traces[1].scale.automatic = false;
        windows[0].traces[1].scale.per_division = 2.;
        open(
            &mut panel,
            &windows[0],
            &graph.node(id).unwrap().config,
            Key::Meas,
            None,
        );
        let mut editor = panel.editor.take().unwrap();
        editor.display.parameter = "S21".into();
        apply_editor(&editor, &mut panel, &mut windows, &mut graph).unwrap();
        assert_eq!(windows[0].traces[0].parameter, "S21");
        assert_eq!(windows[0].traces[1].scale.per_division, 2.);
        open(
            &mut panel,
            &windows[0],
            &graph.node(id).unwrap().config,
            Key::Freq,
            Some(Field::Start),
        );
        let mut editor = panel.editor.take().unwrap();
        editor.config.start_hz = editor.config.stop_hz;
        let before = Snapshot::capture(&panel, &windows, &graph);
        assert!(apply_editor(&editor, &mut panel, &mut windows, &mut graph).is_err());
        assert_eq!(before, Snapshot::capture(&panel, &windows, &graph));
        editor.config = editor.original_config.clone();
        windows[0].traces[0].parameter = "S12".into();
        assert!(apply_editor(&editor, &mut panel, &mut windows, &mut graph).is_err());
    }
    #[test]
    fn front_panel_undo_and_serialization_keep_markers_but_drop_transient_dialogs() {
        let (mut graph, mut panel, mut windows) = bench();
        let id = windows[0].node;
        let before = Snapshot::capture(&panel, &windows, &graph);
        open(
            &mut panel,
            &windows[0],
            &graph.node(id).unwrap().config,
            Key::Scale,
            None,
        );
        let mut editor = panel.editor.take().unwrap();
        editor.display.marker = Some(2.45e9);
        editor.display.scale.per_division = 3.;
        apply_editor(&editor, &mut panel, &mut windows, &mut graph).unwrap();
        panel.editor = Some(editor);
        panel.undo.push(before.clone());
        let loaded: Panel = serde_json::from_str(&serde_json::to_string(&panel).unwrap()).unwrap();
        assert!(loaded.editor.is_none() && loaded.undo.is_empty());
        let loaded_windows: Vec<Window> =
            serde_json::from_str(&serde_json::to_string(&windows).unwrap()).unwrap();
        assert_eq!(loaded_windows[0].traces[0].marker, Some(2.45e9));
        before.clone().restore(&mut panel, &mut windows, &mut graph);
        assert_eq!(before, Snapshot::capture(&panel, &windows, &graph));
    }
    #[test]
    fn receiver_choices_follow_ports_and_source_and_peak_requires_actual_data() {
        let mut c = Config::default();
        c.instrument.port_count = 2;
        c.instrument.pna.power_sweep = true;
        c.instrument.pna.source_port = 2;
        let choices = parameters(&c);
        assert!(choices.contains(&"S12".into()) && choices.contains(&"B_2".into()));
        assert!(!choices.contains(&"S21".into()) && !choices.contains(&"D_2".into()));
        let (_, _, windows) = bench();
        let trace = &windows[0].traces[0];
        assert!(peak(trace, &[], true).is_none());
    }
}
