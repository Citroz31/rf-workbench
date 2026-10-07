use crate::{
    canvas::{Canvas, History, Tool},
    plot,
    shortcuts::{Action, Preferences},
    theme::*,
    visuals::{self, Icon, icon_button},
};
use eframe::egui::{self, RichText};
use rf_core::{Kind, Measurement, NetworkTrace, Project, TestResult, Trace, Waveform};
use rf_dut_library::{Catalog, Component};
use rf_runtime::{Command, Event, RunResult, Worker};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
mod workspace;

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Schematic,
    Acquisition,
    Tests,
    Python,
    Instruments,
    Network,
    Measurements,
    Library,
    DutCatalog,
    Settings,
    Project,
}
pub struct Workbench {
    capture_path: Option<String>,
    capture_frames: u32,
    capture_requested: bool,
    project: Project,
    canvas: Canvas,
    history: History,
    worker: Worker,
    view: View,
    trace: Trace,
    network: Option<NetworkTrace>,
    waveform: Option<Waveform>,
    measurements: Vec<Measurement>,
    preferences: Preferences,
    preferences_draft: Preferences,
    preferences_path: String,
    catalog: Catalog,
    catalog_path: String,
    catalog_search: String,
    component_draft: Component,
    preview: bool,
    tests: Vec<TestResult>,
    logs: VecDeque<(String, bool)>,
    python_path: String,
    script: String,
    resource: String,
    query: String,
    project_path: String,
    csv_path: String,
    hardware: bool,
    continuous: bool,
    marker: bool,
    search: String,
    elapsed: f64,
    sequence: u64,
    frame_ms: f64,
    last_frame: Instant,
    clock: Instant,
    last_status: String,
}
impl Workbench {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup(&cc.egui_ctx);
        let ctx = cc.egui_ctx.clone();
        let worker = Worker::spawn(move || ctx.request_repaint());
        let trace = rf_instruments::simulate_trace(&rf_core::Config::default(), 2.45e9, -13., 0)
            .expect("preview trace");
        let python_path = std::env::var("RF_WORKBENCH_PYTHON")
            .ok()
            .or_else(|| {
                std::env::current_exe()
                    .ok()
                    .and_then(|exe| exe.parent().map(|p| p.join("python-path.txt")))
                    .and_then(|p| std::fs::read_to_string(p).ok())
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or_else(|| "python".into());
        let project_path = std::env::current_dir()
            .unwrap_or_default()
            .join("bench.rfw.json")
            .to_string_lossy()
            .into_owned();
        let csv_path = std::env::current_dir()
            .unwrap_or_default()
            .join("measurement.csv")
            .to_string_lossy()
            .into_owned();
        let preferences_path = std::env::current_dir()
            .unwrap_or_default()
            .join("preferences.rfw.json")
            .to_string_lossy()
            .into_owned();
        let preferences = read_limited(&preferences_path, 64_000)
            .ok()
            .filter(|s| s.len() <= 64_000)
            .and_then(|s| serde_json::from_str::<Preferences>(&s).ok())
            .filter(|p| p.validate().is_ok())
            .unwrap_or_default();
        let catalog_path = std::env::current_dir()
            .unwrap_or_default()
            .join("dut-catalog.json")
            .to_string_lossy()
            .into_owned();
        let catalog = read_limited(&catalog_path, 4_000_000)
            .ok()
            .and_then(|s| Catalog::from_json(&s).ok())
            .unwrap_or_default();
        let args: Vec<_> = std::env::args().collect();
        let mut project = Project::default();
        for n in &mut project.graph.nodes {
            n.position[0] = 36. + (n.position[0] - 36.) / 258. * 294.;
            n.position[1] = if n.id <= 3 { 40. } else { 320. };
        }
        let pna = project.graph.add(Kind::PnaX, [920., 40.]);
        project
            .graph
            .connect_ports(2, 1, pna, 0)
            .expect("model link");
        for e in &mut project.graph.edges {
            e.waypoints = match (e.from, e.to) {
                (2, id) if id == pna => vec![[588., 16.], [896., 16.]],
                (3, 4) => vec![[894., 276.], [600., 276.], [600., 437.5]],
                (4, 5) => vec![[894., 552.], [306., 552.], [306., 437.5]],
                (5, 6) => vec![[600., 576.], [12., 576.], [12., 437.5]],
                _ => Vec::new(),
            };
        }
        if args.iter().any(|a| a == "--demo-pna") {
            project.graph = rf_core::Graph::network_demo();
            project.name = "Caractérisation réseau · PNA-X & DUT".into();
        }
        if args.iter().any(|a| a == "--demo-iq") {
            project.graph = rf_core::Graph::iq_demo();
            project.name = "Chaîne I/Q · AWG, DAC & conversion RF".into();
        }
        let view = if args.iter().any(|a| a == "--gallery") {
            View::Library
        } else if args.iter().any(|a| a == "--settings") {
            View::Settings
        } else {
            View::Schematic
        };
        let mut bench = Self {
            capture_path: args
                .iter()
                .position(|a| a == "--capture")
                .and_then(|i| args.get(i + 1))
                .cloned(),
            capture_frames: 0,
            capture_requested: false,
            project,
            network: None,
            waveform: None,
            measurements: Vec::new(),
            preferences_draft: preferences.clone(),
            preferences: preferences.clone(),
            preferences_path,
            catalog,
            catalog_path,
            catalog_search: String::new(),
            component_draft: Component {
                id: String::new(),
                manufacturer: "Local".into(),
                part_number: String::new(),
                category: "Composant RF".into(),
                description: String::new(),
                insertion_loss_db: Some(3.),
                noise_figure_db: Some(2.5),
                sources: Vec::new(),
                attributes: Default::default(),
            },
            canvas: Canvas::configured(preferences.snap, preferences.orthogonal),
            history: History::default(),
            worker,
            view,
            trace,
            preview: true,
            tests: Vec::new(),
            logs: VecDeque::from([
                (
                    "Banc de démonstration chargé. La courbe est un aperçu simulé.".into(),
                    false,
                ),
                (
                    "Python : régler l'interpréteur dans l'onglet Python, puis Exécuter.".into(),
                    false,
                ),
            ]),
            python_path,
            script: rf_core::Config::default().script,
            resource: "SIM::RF::INSTR".into(),
            query: "*IDN?".into(),
            project_path,
            csv_path,
            hardware: false,
            continuous: false,
            marker: true,
            search: String::new(),
            elapsed: 0.,
            sequence: 0,
            frame_ms: 0.,
            last_frame: Instant::now(),
            clock: Instant::now(),
            last_status: "Prêt · simulation".into(),
        };
        if args.iter().any(|a| a == "--run-demo") {
            bench.run();
            if args.iter().any(|a| a == "--results") {
                bench.view = if args.iter().any(|a| a == "--demo-pna") {
                    View::Network
                } else {
                    View::Measurements
                };
            }
        }
        bench
    }
    fn log(&mut self, message: String, error: bool) {
        self.last_status = message.clone();
        self.logs.push_back((
            format!("{:>6.1}s  {message}", self.clock.elapsed().as_secs_f64()),
            error,
        ));
        while self.logs.len() > 200 {
            self.logs.pop_front();
        }
    }
    fn submit(&mut self, command: Command) {
        match self.worker.submit(command) {
            Ok(()) => self.log("Tâche démarrée en arrière-plan".into(), false),
            Err(e) => self.log(e, true),
        }
    }
    fn run(&mut self) {
        self.canvas.completed.clear();
        self.tests.clear();
        self.submit(Command::Run {
            graph: self.project.graph.clone(),
            continuous: self.continuous,
            python_path: self.python_path.clone(),
            hardware: self.hardware,
        });
    }
    fn result(&mut self, r: RunResult) {
        if let Some(t) = r.trace {
            self.trace = t;
            self.preview = false;
        }
        self.network = r.network;
        self.waveform = r.waveform;
        self.measurements = r.measurements;
        self.tests = r.tests;
        self.canvas.completed = r.completed;
        self.elapsed = r.elapsed_ms;
        self.sequence = r.sequence + 1;
    }
    fn poll(&mut self) {
        if let Some(r) = self.worker.take_latest() {
            self.result(r);
        }
        let events: Vec<_> = self.worker.events.try_iter().collect();
        for event in events {
            match event {
                Event::Done(r) => {
                    self.result(*r);
                    self.log("Traitement terminé".into(), false);
                }
                Event::Suite(t) => {
                    let passed = t.iter().filter(|v| v.passed).count();
                    self.log(
                        format!("Autotests : {passed}/{} réussis", t.len()),
                        passed != t.len(),
                    );
                    self.tests = t;
                }
                Event::Message(s) => self.log(s, false),
                Event::Error(e) => self.log(e, true),
                Event::Idle => {
                    if !self.last_status.contains("Erreur") {
                        self.log(
                            "Tâche terminée ; consulter les résultats et le journal".into(),
                            false,
                        );
                    }
                }
            }
        }
    }
    fn save(&mut self) {
        let result = self
            .project
            .to_json()
            .map_err(|e| e.to_string())
            .and_then(|s| atomic_save(&self.project_path, &s));
        match result {
            Ok(()) => self.log(format!("Projet enregistré : {}", self.project_path), false),
            Err(e) => self.log(e, true),
        }
    }
    fn load(&mut self) {
        let result = std::fs::metadata(&self.project_path)
            .map_err(|e| e.to_string())
            .and_then(|m| {
                if m.len() > 4_000_000 {
                    Err("Projet trop volumineux".into())
                } else {
                    std::fs::read_to_string(&self.project_path).map_err(|e| e.to_string())
                }
            })
            .and_then(|s| Project::from_json(&s).map_err(|e| e.to_string()));
        match result {
            Ok(p) => {
                self.project = p;
                self.history.clear();
                self.canvas =
                    Canvas::configured(self.preferences.snap, self.preferences.orthogonal);
                self.network = None;
                self.waveform = None;
                self.measurements.clear();
                self.tests.clear();
                self.preview = true;
                self.canvas.completed.clear();
                self.log(
                    "Projet ouvert ; exécuter pour acquérir une nouvelle trace".into(),
                    false,
                );
            }
            Err(e) => self.log(e, true),
        }
    }
    fn export(&mut self) {
        match self
            .trace
            .csv()
            .map_err(|e| e.to_string())
            .and_then(|s| atomic_save(&self.csv_path, &s))
        {
            Ok(()) => self.log(format!("Trace exportée : {}", self.csv_path), false),
            Err(e) => self.log(e, true),
        }
    }
    fn add_block(&mut self, kind: Kind) {
        if self.project.graph.nodes.len() >= 1000 {
            self.log("Limite de 1000 blocs atteinte".into(), true);
            return;
        }
        self.history.record(self.project.graph.clone());
        let n = self.project.graph.nodes.len() as f32;
        let id = self
            .project
            .graph
            .add(kind, [36. + (n % 4.) * 294., 40. + (n / 4.).floor() * 280.]);
        self.canvas.selected = Some(id);
        self.canvas.completed.clear();
        self.canvas.fit();
        self.view = View::Schematic;
    }
    fn action(&mut self, action: Action) {
        match action {
            Action::Select => self.canvas.set_tool(Tool::Select),
            Action::Wire => {
                self.view = View::Schematic;
                self.canvas.set_tool(if self.canvas.tool == Tool::Wire {
                    Tool::Select
                } else {
                    Tool::Wire
                });
            }
            Action::Pan => self.canvas.set_tool(Tool::Pan),
            Action::Fit => self.canvas.fit(),
            Action::Run => {
                if !self.worker.is_busy() {
                    self.run();
                }
            }
            Action::Stop => self.worker.stop(),
            Action::Save => {
                if !self.worker.is_busy() {
                    self.save();
                }
            }
            Action::Open => {
                if !self.worker.is_busy() {
                    self.load();
                }
            }
            Action::Undo => {
                self.canvas.cancel_wire();
                self.history.undo(&mut self.project.graph);
                self.canvas.completed.clear();
            }
            Action::Redo => {
                self.canvas.cancel_wire();
                self.history.redo(&mut self.project.graph);
                self.canvas.completed.clear();
            }
            Action::Delete => {
                if self.view == View::Schematic
                    && let Some(id) = self.canvas.selected
                {
                    self.history.record(self.project.graph.clone());
                    self.project.graph.remove(id);
                    self.canvas.selected = None;
                    self.canvas.cancel_wire();
                }
            }
            Action::Duplicate => {
                if let Some(old) = self
                    .canvas
                    .selected
                    .and_then(|id| self.project.graph.node(id))
                    .cloned()
                    && self.project.graph.nodes.len() < 1000
                {
                    self.history.record(self.project.graph.clone());
                    let id = self
                        .project
                        .graph
                        .add(old.kind, [old.position[0] + 36., old.position[1] + 36.]);
                    let n = self.project.graph.nodes.last_mut().unwrap();
                    n.config = old.config;
                    n.title = format!("{} · copie", old.title);
                    self.canvas.selected = Some(id);
                }
            }
            Action::Library => self.view = View::Library,
            Action::Settings => self.view = View::Settings,
            Action::Network => self.view = View::Network,
            Action::Measurements => self.view = View::Measurements,
            Action::Export => self.export(),
        }
    }
    fn top(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(5.);
            ui.horizontal(|ui| {
                ui.label(RichText::new("RF").size(24.).strong().color(TEAL));
                ui.label(RichText::new("WORKBENCH").size(19.).strong());
                ui.label(
                    RichText::new("/ Instrumentation & métrologie")
                        .size(12.)
                        .color(MUTED),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    badge(ui, "v0.2 · WINDOWS", MUTED);
                    badge(
                        ui,
                        if self.hardware {
                            "MATÉRIEL ACTIVÉ"
                        } else {
                            "SIMULATION"
                        },
                        if self.hardware { GOLD } else { TEAL },
                    );
                });
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                for (view, icon, name) in [
                    (View::Schematic, Icon::Wire, "Schéma"),
                    (View::Library, Icon::Grid, "Blocs"),
                    (View::Acquisition, Icon::Wave, "Spectre"),
                    (View::Network, Icon::Network, "Paramètres S"),
                    (View::Measurements, Icon::Wave, "Mesures"),
                    (View::Tests, Icon::Test, "Tests"),
                    (View::Python, Icon::Python, "Python"),
                    (View::Instruments, Icon::Instrument, "SCPI"),
                    (View::DutCatalog, Icon::Chip, "DUT"),
                    (View::Project, Icon::Open, "Projet"),
                    (View::Settings, Icon::Settings, "Raccourcis"),
                ] {
                    ui.spacing_mut().item_spacing.x = 4.;
                    if icon_button(ui, icon, name, self.view == view).clicked() {
                        self.view = view;
                    }
                    if ui
                        .selectable_label(self.view == view, RichText::new(name).size(12.))
                        .clicked()
                    {
                        self.view = view;
                    }
                    ui.add_space(6.);
                }
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                for (action, icon, selected) in [
                    (
                        Action::Select,
                        Icon::Select,
                        self.canvas.tool == Tool::Select,
                    ),
                    (Action::Wire, Icon::Wire, self.canvas.tool == Tool::Wire),
                    (Action::Pan, Icon::Pan, self.canvas.tool == Tool::Pan),
                    (Action::Fit, Icon::Fit, false),
                ] {
                    if icon_button(ui, icon, &self.preferences.hint(action), selected).clicked() {
                        self.action(action);
                    }
                }
                ui.separator();
                for (action, icon) in [
                    (Action::Undo, Icon::Undo),
                    (Action::Redo, Icon::Redo),
                    (Action::Duplicate, Icon::Clone),
                    (Action::Save, Icon::Save),
                    (Action::Open, Icon::Open),
                    (Action::Export, Icon::Export),
                ] {
                    if icon_button(ui, icon, &self.preferences.hint(action), false).clicked() {
                        self.action(action);
                    }
                }
                ui.separator();
                let busy = self.worker.is_busy();
                ui.add_enabled_ui(!busy, |ui| {
                    if icon_button(ui, Icon::Play, &self.preferences.hint(Action::Run), false)
                        .clicked()
                    {
                        self.run();
                    }
                    if button(ui, "Exécuter", true)
                        .on_hover_text(self.preferences.hint(Action::Run))
                        .clicked()
                    {
                        self.run();
                    }
                    ui.checkbox(&mut self.continuous, "Continu");
                });
                ui.add_enabled_ui(busy, |ui| {
                    if icon_button(ui, Icon::Stop, &self.preferences.hint(Action::Stop), false)
                        .clicked()
                    {
                        self.worker.stop();
                    }
                });
                ui.separator();
                if icon_button(ui, Icon::Grid, "Aimantation à la grille", self.canvas.snap)
                    .clicked()
                {
                    self.canvas.snap = !self.canvas.snap;
                    self.preferences_draft.snap = self.canvas.snap;
                }
                if ui
                    .checkbox(&mut self.canvas.orthogonal, "Câbles orthogonaux")
                    .changed()
                {
                    self.preferences_draft.orthogonal = self.canvas.orthogonal;
                }
                ui.add_enabled_ui(!busy, |ui| {
                    ui.checkbox(&mut self.hardware, "Matériel réel");
                });
            });
            ui.add_space(3.);
        });
    }
    fn sidebar(&mut self, ctx: &egui::Context) {
        let query = self.search.to_lowercase();
        egui::SidePanel::left("library")
            .resizable(true)
            .default_width(232.)
            .width_range(190. ..=310.)
            .show(ctx, |ui| {
                ui.add_space(10.);
                caption(ui, "BIBLIOTHÈQUE DE BLOCS");
                ui.label(
                    RichText::new("18 éléments · ports nommés")
                        .size(11.)
                        .color(MUTED),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("PNA, DAC, thermique…")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(6.);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for category in [
                        "Instruments RF",
                        "Électronique & conversion",
                        "Thermique",
                        "Composants DUT",
                        "Analyse & automatisation",
                    ] {
                        egui::CollapsingHeader::new(category)
                            .default_open(true)
                            .show(ui, |ui| {
                                for k in Kind::ALL.into_iter().filter(|k| {
                                    k.category() == category
                                        && k.label().to_lowercase().contains(&query)
                                }) {
                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::vec2(ui.available_width(), 48.),
                                        egui::Sense::click(),
                                    );
                                    let p = ui.painter();
                                    p.rect_filled(
                                        rect,
                                        5.,
                                        if response.hovered() { BORDER } else { CARD },
                                    );
                                    visuals::symbol(
                                        p,
                                        egui::Rect::from_min_size(
                                            rect.min + egui::vec2(4., 5.),
                                            egui::vec2(57., 36.),
                                        ),
                                        k,
                                    );
                                    p.text(
                                        rect.min + egui::vec2(66., 18.),
                                        egui::Align2::LEFT_CENTER,
                                        k.label(),
                                        egui::FontId::proportional(12.),
                                        crate::theme::kind(k),
                                    );
                                    p.text(
                                        rect.min + egui::vec2(66., 34.),
                                        egui::Align2::LEFT_CENTER,
                                        "Ajouter au banc",
                                        egui::FontId::proportional(10.),
                                        MUTED,
                                    );
                                    if response
                                        .on_hover_text(format!(
                                            "{} · {} entrées / {} sorties",
                                            k.label(),
                                            k.inputs().len(),
                                            k.outputs().len()
                                        ))
                                        .clicked()
                                    {
                                        self.add_block(k);
                                    }
                                }
                            });
                    }
                    ui.add_space(12.);
                    ui.label(
                        RichText::new(
                            "W : câblage · V : sélection
H : déplacement · F : cadrage
Les touches sont personnalisables.",
                        )
                        .size(11.)
                        .color(MUTED),
                    );
                });
            });
    }
    fn inspector(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("inspector").resizable(true).default_width(270.).width_range(240. ..=400.).show(ctx,|ui|{egui::ScrollArea::vertical().show(ui,|ui|{
            ui.add_space(10.);caption(ui,"INSPECTEUR");let mut old=None;let mut changed=false;let mut remove=None;let mut open_python=false;
            if let Some(id)=self.canvas.selected && let Some(n)=self.project.graph.nodes.iter_mut().find(|n|n.id==id){
                old=Some(n.clone());badge(ui,n.kind.tag(),crate::theme::kind(n.kind));changed|=ui.text_edit_singleline(&mut n.title).changed();
                let (r,_)=ui.allocate_exact_size(egui::vec2(ui.available_width(),90.),egui::Sense::hover());visuals::symbol(ui.painter(),r.shrink(8.),n.kind);ui.separator();let c=&mut n.config;
                match n.kind {
                    Kind::Generator=>{changed|=number(ui,"FRÉQUENCE",&mut c.frequency_hz,1e6," Hz");changed|=number(ui,"PUISSANCE",&mut c.power_dbm,0.1," dBm");},
                    Kind::Dut=>{changed|=number(ui,"PERTE D'INSERTION",&mut c.loss_db,0.1," dB");changed|=number(ui,"FACTEUR DE BRUIT",&mut c.noise_figure_db,0.1," dB");ui.label(c.dut_id.as_deref().unwrap_or("DUT générique · modèle local"));if ui.button("Ouvrir le catalogue DUT").clicked(){self.view=View::DutCatalog;}ui.label(RichText::new("MODEL fournit un modèle au PNA/NF Meter. RF IN/OUT représente la chaîne de signal.").size(11.).color(MUTED));},
                    Kind::Analyzer|Kind::Pna|Kind::PnaX=>{changed|=number(ui,"DÉBUT BALAYAGE",&mut c.start_hz,1e6," Hz");changed|=number(ui,"FIN BALAYAGE",&mut c.stop_hz,1e6," Hz");caption(ui,"POINTS");changed|=ui.add(egui::DragValue::new(&mut c.points).range(2..=rf_core::MAX_POINTS)).changed();if n.kind==Kind::Analyzer{caption(ui,"REQUÊTE TRACE ASCII");changed|=ui.text_edit_singleline(&mut c.trace_query).changed();}else{caption(ui,"PARAMÈTRE S");egui::ComboBox::from_id_salt("s-param").selected_text(&c.s_parameter).show_ui(ui,|ui|{for p in ["S11","S21","S12","S22"]{changed|=ui.selectable_value(&mut c.s_parameter,p.into(),p).changed();}});}},
                    Kind::Awg|Kind::Dac|Kind::Adc=>{changed|=number(ui,"CADENCE AWG",&mut c.sample_rate_hz,1e6," Sa/s");changed|=number(ui,"FRÉQUENCE DE BASE",&mut c.tone_hz,1e4," Hz");caption(ui,"ÉCHANTILLONS AWG");changed|=ui.add(egui::DragValue::new(&mut c.samples).range(2..=65536)).changed();caption(ui,"RÉSOLUTION");changed|=ui.add(egui::DragValue::new(&mut c.resolution_bits).range(2..=24).suffix(" bits")).changed();changed|=number(ui,"PLEINE ÉCHELLE",&mut c.voltage_v,0.05," V");},
                    Kind::IqModulator=>{changed|=number(ui,"PERTE CONVERSION",&mut c.loss_db,0.1," dB");ui.label("Entrées I / Q analogiques et oscillateur LO. Modèle idéal de transposition.");},
                    Kind::VariableResistor=>{changed|=number(ui,"RÉSISTANCE",&mut c.resistance_ohm,1.," Ω");},
                    Kind::Thermometer|Kind::Thermostream=>{changed|=number(ui,"TEMPÉRATURE / CONSIGNE",&mut c.temperature_c,0.1," °C");ui.label("Thermostream : consigne idéale sans dynamique thermique. Thermomètre : entrée TEMP ou valeur locale.");},
                    Kind::NoiseFigureMeter=>{changed|=number(ui,"FACTEUR DE BRUIT",&mut c.noise_figure_db,0.1," dB");},
                    Kind::PowerSensor|Kind::PowerMeter=>{ui.label("Mesure scalaire en dBm. Relier RF → Power Sensor → Power Meter.");},
                    Kind::Python=>{if ui.button("Éditer le script Python").clicked(){self.script=c.script.clone();open_python=true;}ui.label("Entrée trace / sortie output");},Kind::Peak=>{ui.label("Maximum de trace en dBm");},
                    Kind::Limit=>{changed|=number(ui,"LIMITE BASSE",&mut c.lower_dbm,0.1," dBm");changed|=number(ui,"LIMITE HAUTE",&mut c.upper_dbm,0.1," dBm");},
                }
                if matches!(n.kind,Kind::Generator|Kind::Analyzer)||n.kind.is_extended(){caption(ui,"RESSOURCE VISA / SCPI");changed|=ui.text_edit_singleline(&mut c.resource).changed();}
                if n.kind.is_extended(){ui.label(RichText::new("Modèle simulé dans v0.2. Les profils matériels spécifiques restent à développer.").size(11.).color(GOLD));}
                ui.separator();caption(ui,"PORTS DE DONNÉES");for p in n.kind.inputs(){ui.colored_label(port(p.port),format!("IN {} · {}{}",p.name,p.port.label(),if p.required{" · requis"}else{" · optionnel"}));}for p in n.kind.outputs(){ui.colored_label(port(p.port),format!("OUT {} · {}",p.name,p.port.label()));}
                if ui.button(RichText::new("Supprimer le bloc").color(RED)).clicked(){remove=Some(id);}
            }else{ui.label("Sélectionner un bloc.");}
            if changed && let Some(old)=old{let mut before=self.project.graph.clone();if let Some(n)=before.nodes.iter_mut().find(|n|n.id==old.id){*n=old;}self.history.record(before);self.canvas.completed.clear();}
            if let Some(id)=remove{self.history.record(self.project.graph.clone());self.project.graph.remove(id);self.canvas.selected=None;self.canvas.cancel_wire();}
if open_python{self.view=View::Python;}
        });});
    }
    fn bottom(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status")
            .exact_height(30.)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    badge(
                        ui,
                        if self.worker.is_busy() {
                            "●  EN COURS"
                        } else {
                            "●  PRÊT"
                        },
                        TEAL,
                    );
                    ui.label(
                        RichText::new(format!(
                            "{} blocs  ·  {} câbles",
                            self.project.graph.nodes.len(),
                            self.project.graph.edges.len()
                        ))
                        .size(11.)
                        .color(MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "CPU interface {:.2} ms  ·  cycle {:.1} ms  ·  #{}",
                                self.frame_ms, self.elapsed, self.sequence
                            ))
                            .monospace()
                            .size(11.)
                            .color(MUTED),
                        );
                    });
                });
            });
        if !self.preferences.show_journal {
            return;
        }
        egui::TopBottomPanel::bottom("journal")
            .resizable(true)
            .default_height(85.)
            .height_range(70. ..=350.)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    caption(ui, "JOURNAL D'EXÉCUTION");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Effacer").clicked() {
                            self.logs.clear();
                        }
                    });
                });
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for (msg, error) in &self.logs {
                            ui.label(RichText::new(msg).monospace().size(11.).color(if *error {
                                RED
                            } else {
                                MUTED
                            }));
                        }
                    });
            });
    }
    fn center(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(PANEL).inner_margin(18.))
            .show(ctx, |ui| {
                ui.heading(&self.project.name);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Banc RF / Spectre & conformité")
                            .size(12.)
                            .color(MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        badge(
                            ui,
                            if self.preview {
                                "APERÇU SIMULÉ"
                            } else if self.trace.simulated {
                                "DONNÉES SIMULÉES"
                            } else {
                                "MESURE MATÉRIELLE"
                            },
                            if self.trace.simulated { TEAL } else { GOLD },
                        );
                    });
                });
                ui.add_space(9.);
                match self.view {
                    View::Schematic => {
                        ui.horizontal(|ui| {
                            caption(ui, "SCHÉMA DU BANC");
                            if ui.small_button("Ajuster").clicked() {
                                self.canvas.fit();
                            }
                            ui.label(
                                RichText::new(format!("{:.0}%", self.canvas.zoom * 100.))
                                    .size(11.)
                                    .color(MUTED),
                            );
                            if let Err(e) = self.project.graph.validate() {
                                ui.label(RichText::new(e.to_string()).size(11.).color(GOLD));
                            }
                        });
                        let height = (ui.available_height() - 195.).max(260.);
                        if let Some(error) =
                            self.canvas
                                .show(ui, &mut self.project.graph, &mut self.history, height)
                        {
                            self.log(error, true);
                        }
                        ui.add_space(12.);
                        self.trace_header(ui);
                        plot::plot(ui, &self.trace, 145., self.marker);
                    }
                    View::Acquisition => {
                        self.trace_header(ui);
                        if let Ok((freq, power)) = self.trace.peak() {
                            ui.horizontal(|ui| {
                                metric(ui, "PIC", &format!("{power:.3} dBm"), TEAL);
                                metric(ui, "FRÉQUENCE", &format!("{:.6} GHz", freq / 1e9), BLUE);
                                metric(
                                    ui,
                                    "POINTS",
                                    &self.trace.amplitude_dbm.len().to_string(),
                                    TEXT,
                                );
                            });
                        }
                        plot::plot(
                            ui,
                            &self.trace,
                            (ui.available_height() - 80.).max(200.),
                            self.marker,
                        );
                        ui.horizontal(|ui| {
                            ui.label("La courbe suit la dernière acquisition complète.");
                            if ui.button("Exporter CSV").clicked() {
                                self.export();
                            }
                        });
                    }
                    View::Tests => self.test_view(ui),
                    View::Python => self.python_view(ui),
                    View::Instruments => self.instrument_view(ui),
                    View::Network => self.network_view(ui),
                    View::Measurements => self.measurement_view(ui),
                    View::Library => self.library_view(ui),
                    View::DutCatalog => self.catalog_view(ui),
                    View::Settings => self.settings_view(ui),
                    View::Project => self.project_view(ui),
                }
            });
    }
    fn trace_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            caption(ui, "SPECTRE RF");
            ui.label(
                RichText::new("Amplitude / fréquence")
                    .size(11.)
                    .color(MUTED),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(&mut self.marker, "Marqueur M1");
            });
        });
    }
    fn test_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.worker.is_busy(),
                    egui::Button::new("Lancer les autotests"),
                )
                .clicked()
            {
                self.submit(Command::Suite);
            }
            if ui
                .add_enabled(!self.worker.is_busy(), egui::Button::new("Tester le banc"))
                .clicked()
            {
                self.run();
            }
        });
        ui.label(RichText::new("Les autotests vérifient le logiciel. Tester le banc évalue les limites définies par ses blocs.").color(MUTED));
        ui.add_space(12.);
        if self.tests.is_empty() {
            ui.label("Aucun résultat. Lancer une série de tests.");
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for t in &self.tests {
                egui::Frame::new()
                    .fill(CARD)
                    .inner_margin(16.)
                    .corner_radius(8.)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            badge(
                                ui,
                                if t.passed { "PASS" } else { "FAIL" },
                                if t.passed { TEAL } else { RED },
                            );
                            ui.strong(&t.name);
                        });
                        ui.label(RichText::new(&t.detail).color(MUTED));
                    });
                ui.add_space(4.);
            }
        });
    }
    fn python_view(&mut self, ui: &mut egui::Ui) {
        caption(ui, "INTERPRÉTEUR PYTHON");
        ui.add(
            egui::TextEdit::singleline(&mut self.python_path)
                .desired_width(f32::INFINITY)
                .hint_text("C:\\Python312\\python.exe"),
        );
        ui.label(RichText::new("API : trace, output, ResourceManager().open_resource(...).query(...) / .write(...)").size(12.).color(MUTED));
        ui.horizontal(|ui| {
            if ui.button("Charger l'exemple").clicked() {
                self.script = include_str!("../../../python/examples/cable_compensation.py").into();
            }
            if ui.button("Appliquer au bloc sélectionné").clicked() {
                let before = self.project.graph.clone();
                if let Some(node) = self
                    .project
                    .graph
                    .nodes
                    .iter_mut()
                    .find(|n| Some(n.id) == self.canvas.selected && n.kind == Kind::Python)
                {
                    node.config.script = self.script.clone();
                    self.history.record(before);
                    self.log("Script enregistré dans le bloc Python".into(), false);
                } else {
                    self.log("Sélectionner un bloc Python dans le schéma".into(), true);
                }
            }
            if ui
                .add_enabled(
                    !self.worker.is_busy(),
                    egui::Button::new("Exécuter sur la trace"),
                )
                .clicked()
            {
                self.submit(Command::Python {
                    source: self.script.clone(),
                    trace: self.trace.clone(),
                    python_path: self.python_path.clone(),
                    hardware: self.hardware,
                });
            }
        });
        ui.add_space(8.);
        egui::ScrollArea::both()
            .max_height((ui.available_height() - 110.).max(200.))
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.script)
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .desired_rows(22)
                        .lock_focus(true),
                );
            });
        ui.add_space(12.);
        ui.label(RichText::new("Le processus est supervisé et interrompu après 5 s (hors requête instrument déjà en cours). Python dispose des permissions de ton compte ; exécuter uniquement des scripts de confiance.").size(12.).color(MUTED));
    }
    fn instrument_view(&mut self, ui: &mut egui::Ui) {
        caption(ui, "GESTIONNAIRE DE RESSOURCES");
        ui.label("Simulation disponible ; saisir explicitement l'adresse d'un instrument TCP.");
        ui.add_space(8.);
        ui.label("Ressource");
        ui.add(egui::TextEdit::singleline(&mut self.resource).desired_width(f32::INFINITY));
        ui.label("Commande SCPI (query)");
        ui.add(egui::TextEdit::singleline(&mut self.query).desired_width(f32::INFINITY));
        if ui
            .add_enabled(
                !self.worker.is_busy(),
                egui::Button::new("Interroger l'instrument"),
            )
            .clicked()
        {
            self.submit(Command::Query {
                resource: self.resource.clone(),
                command: self.query.clone(),
                hardware: self.hardware,
            });
        }
        ui.add_space(20.);
        egui::Grid::new("transports")
            .spacing([25., 16.])
            .show(ui, |ui| {
                ui.strong("Transport");
                ui.strong("État");
                ui.end_row();
                ui.label("Simulation SCPI");
                badge(ui, "Disponible", TEAL);
                ui.end_row();
                ui.label("SCPI TCP / SOCKET");
                badge(ui, "Disponible · timeout 2 s", BLUE);
                ui.end_row();
                ui.label("USB / GPIB / série via VISA");
                badge(ui, "Adaptateur · runtime constructeur requis", MUTED);
                ui.end_row();
                ui.label("VXI-11 / HiSLIP");
                badge(ui, "Via runtime VISA", MUTED);
                ui.end_row();
            });
        ui.add_space(16.);
        ui.label(RichText::new("Le pilote d'analyseur initial utilise un profil SCPI générique et une trace ASCII en dBm. Vérifier les commandes dans le manuel du modèle. La découverte réseau automatique et les pilotes constructeur ne sont pas encore intégrés.").color(MUTED));
    }
}
fn metric(ui: &mut egui::Ui, label: &str, value: &str, color: egui::Color32) {
    egui::Frame::new()
        .fill(CARD)
        .inner_margin(14.)
        .corner_radius(8.)
        .show(ui, |ui| {
            ui.set_min_width(160.);
            caption(ui, label);
            ui.label(RichText::new(value).size(24.).strong().color(color));
        });
}
fn number(ui: &mut egui::Ui, label: &str, value: &mut f64, speed: f64, unit: &str) -> bool {
    caption(ui, label);
    ui.add(egui::DragValue::new(value).speed(speed).suffix(unit))
        .changed()
}
fn read_limited(path: &str, max: u64) -> Result<String, String> {
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if metadata.len() > max {
        return Err("Fichier trop volumineux".into());
    }
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

fn atomic_save(path: &str, text: &str) -> Result<(), String> {
    use std::io::Write;
    let dest = std::path::Path::new(path);
    let parent = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(dest).map_err(|e| e.to_string())?;
    Ok(())
}
impl eframe::App for Workbench {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let start = Instant::now();
        self.poll();
        let events = ctx.input(|i| i.events.clone());
        for event in &events {
            if let egui::Event::Screenshot { image, .. } = event
                && let Some(path) = self.capture_path.take()
            {
                let pixels: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                let result = image::save_buffer_with_format(
                    &path,
                    &pixels,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                );
                if let Err(e) = result {
                    self.log(e.to_string(), true);
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        let typing = ctx.wants_keyboard_input();
        for event in &events {
            if let Some(action) = self.preferences.action(event, typing) {
                self.action(action);
            }
        }
        self.top(ctx);
        self.bottom(ctx);
        self.sidebar(ctx);
        if matches!(
            self.view,
            View::Schematic | View::Acquisition | View::Network | View::Measurements
        ) {
            self.inspector(ctx);
        }
        self.center(ctx);
        self.frame_ms = self.frame_ms * 0.9 + start.elapsed().as_secs_f64() * 1000. * 0.1;
        self.last_frame = Instant::now();
        if self.capture_path.is_some() {
            self.capture_frames += 1;
            if self.capture_frames >= 5 && !self.capture_requested {
                self.capture_requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            ctx.request_repaint_after(Duration::from_millis(30));
        }
        if self.worker.is_busy() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saving_replaces_existing_file_without_losing_new_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bench.rfw.json");
        atomic_save(path.to_str().unwrap(), "first").unwrap();
        atomic_save(path.to_str().unwrap(), "second").unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "second");
    }
    #[test]
    fn failed_save_preserves_existing_directory() {
        let dir = tempfile::tempdir().unwrap();
        assert!(atomic_save(dir.path().to_str().unwrap(), "content").is_err());
        assert!(dir.path().is_dir());
    }
}
