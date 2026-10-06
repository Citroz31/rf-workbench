use crate::{
    canvas::{Canvas, History},
    plot,
    theme::*,
};
use eframe::egui::{self, RichText};
use rf_core::{Kind, Project, TestResult, Trace};
use rf_runtime::{Command, Event, RunResult, Worker};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Schematic,
    Acquisition,
    Tests,
    Python,
    Instruments,
}
pub struct Workbench {
    project: Project,
    canvas: Canvas,
    history: History,
    worker: Worker,
    view: View,
    trace: Trace,
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
        Self {
            project: Project::default(),
            canvas: Canvas::default(),
            history: History::default(),
            worker,
            view: View::Schematic,
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
        }
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
                    self.result(r);
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
                self.canvas = Canvas::default();
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
    fn top(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header").exact_height(105.).show(ctx,|ui|{
            ui.add_space(8.);ui.horizontal(|ui|{
                ui.label(RichText::new("RF").size(24.).strong().color(TEAL));
                ui.label(RichText::new("WORKBENCH").size(19.).strong());
                ui.label(RichText::new("/  Instrumentation & métrologie").size(13.).color(MUTED));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{badge(ui,"v0.1 · WINDOWS",MUTED);badge(ui,if self.hardware{"MATÉRIEL ACTIVÉ"}else{"SIMULATION"},if self.hardware{GOLD}else{TEAL});});
            });
            ui.add_space(8.);ui.separator();
            ui.horizontal(|ui|{
                let busy=self.worker.is_busy();
                ui.add_enabled_ui(!busy,|ui|{if button(ui,"▶  Exécuter",true).clicked(){self.run();}ui.checkbox(&mut self.continuous,"En continu");});
                if ui.add_enabled(busy,egui::Button::new(RichText::new("■  Arrêter").color(RED))).clicked(){self.worker.stop();self.log("Arrêt demandé ; attente de la fin de l'opération en cours".into(),false);}
                ui.separator();
                if ui.button("Annuler").on_hover_text("Ctrl+Z").clicked(){self.history.undo(&mut self.project.graph);}
                if ui.button("Rétablir").on_hover_text("Ctrl+Y").clicked(){self.history.redo(&mut self.project.graph);}
                ui.add_enabled_ui(!busy,|ui|{
                    if ui.button("Enregistrer").clicked(){self.save();}
                    if ui.button("Ouvrir").clicked(){self.load();}
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{ui.add_enabled_ui(!busy,|ui|{ui.checkbox(&mut self.hardware,"Matériel réel").on_hover_text("Autorise les sessions SCPI TCP configurées. Le banc de démonstration reste simulé.");});});
            });
        });
    }
    fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("library").resizable(true).default_width(218.).width_range(180. ..=300.).show(ctx,|ui|{
            ui.add_space(14.);caption(ui,"ESPACE DE TRAVAIL");
            for(view,name)in [(View::Schematic,"Schéma du banc"),(View::Acquisition,"Acquisitions"),(View::Tests,"Tests & limites"),(View::Python,"{ }  Python"),(View::Instruments,"Instruments")]{
                let selected=self.view==view;
                if ui.add_sized([ui.available_width(),34.],egui::Button::new(RichText::new(name).color(if selected{TEAL}else{TEXT})).selected(selected)).clicked(){self.view=view;}
            }
            ui.add_space(16.);ui.separator();ui.add_space(12.);caption(ui,"BIBLIOTHÈQUE DE BLOCS");
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Rechercher un bloc…").desired_width(f32::INFINITY));ui.add_space(5.);
            for kind in Kind::ALL {
                if !kind.label().to_lowercase().contains(&self.search.to_lowercase()){continue;}
                let response=ui.add_sized([ui.available_width(),40.],egui::Button::new(RichText::new(format!("+  {}",kind.label())).color(crate::theme::kind(kind))));
                if response.clicked(){self.history.record(self.project.graph.clone());let n=self.project.graph.nodes.len()as f32;let id=self.project.graph.add(kind,[80.+(n%3.)*258.,80.+(n/3.).floor()*210.]);self.canvas.selected=Some(id);self.canvas.fit();self.view=View::Schematic;self.log("Bloc ajouté : relier ses ports avant l'exécution".into(),false);}
            }
            ui.add_space(20.);caption(ui,"RESSOURCES DU BANC");
            badge(ui,"●  SIM::RF::INSTR",TEAL);ui.label(RichText::new("Instrument virtuel disponible").size(11.).color(MUTED));
            ui.add_space(15.);ui.label(RichText::new("RF  Signal   ·   Trace   ·   dBm").size(11.).color(MUTED));
            ui.add_space(10.);ui.label(RichText::new("Les blocs sont exécutés selon les câbles. Les changements pendant une acquisition prennent effet au prochain démarrage.").size(12.).color(MUTED));
        });
    }
    fn inspector(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("inspector").resizable(true).default_width(285.).width_range(240. ..=400.).show(ctx,|ui|{
            egui::ScrollArea::vertical().show(ui,|ui|{
                ui.add_space(14.);caption(ui,"INSPECTEUR");
                let mut previous_node=None;let mut changed=false;let mut remove=None;let mut open_python=false;
                if let Some(id)=self.canvas.selected && let Some(n)=self.project.graph.nodes.iter_mut().find(|n|n.id==id){
                    previous_node=Some(n.clone());
                    badge(ui,n.kind.tag(),crate::theme::kind(n.kind));
                    changed|=ui.text_edit_singleline(&mut n.title).changed();ui.add_space(7.);ui.separator();ui.add_space(8.);
                    let c=&mut n.config;
                    match n.kind {
                        Kind::Generator=>{
                            caption(ui,"FRÉQUENCE PORTEUSE");changed|=ui.add(egui::DragValue::new(&mut c.frequency_hz).speed(1e6).range(1. ..=1e12).suffix(" Hz")).changed();
                            caption(ui,"PUISSANCE");changed|=ui.add(egui::DragValue::new(&mut c.power_dbm).speed(0.1).range(-160. ..=30.).suffix(" dBm")).changed();
                        }
                        Kind::Dut=>{caption(ui,"PERTE D'INSERTION");changed|=ui.add(egui::DragValue::new(&mut c.loss_db).speed(0.1).range(0. ..=160.).suffix(" dB")).changed();ui.label(RichText::new("En simulation, cette perte modifie la trace. Sur matériel, le DUT appartient au câblage physique.").size(12.).color(MUTED));}
                        Kind::Analyzer=>{
                            caption(ui,"DÉBUT DU BALAYAGE");changed|=ui.add(egui::DragValue::new(&mut c.start_hz).speed(1e6).suffix(" Hz")).changed();
                            caption(ui,"FIN DU BALAYAGE");changed|=ui.add(egui::DragValue::new(&mut c.stop_hz).speed(1e6).suffix(" Hz")).changed();
                            caption(ui,"POINTS");changed|=ui.add(egui::DragValue::new(&mut c.points).range(2..=rf_core::MAX_POINTS)).changed();
                            caption(ui,"REQUÊTE TRACE ASCII");changed|=ui.text_edit_singleline(&mut c.trace_query).changed();
                        }
                        Kind::Python=>{ui.label("Entrée : trace\nSortie : variable output");if ui.button("Éditer le script Python").clicked(){self.script=c.script.clone();open_python=true;}ui.label(RichText::new("Le script s'exécute dans un processus Python supervisé, via le moteur Rust.").size(12.).color(MUTED));}
                        Kind::Peak=>{ui.label("Renvoie le maximum de la trace en dBm.");}
                        Kind::Limit=>{caption(ui,"LIMITE BASSE");changed|=ui.add(egui::DragValue::new(&mut c.lower_dbm).speed(0.1).suffix(" dBm")).changed();caption(ui,"LIMITE HAUTE");changed|=ui.add(egui::DragValue::new(&mut c.upper_dbm).speed(0.1).suffix(" dBm")).changed();}
                    }
                    if matches!(n.kind,Kind::Generator|Kind::Analyzer){
                        ui.add_space(8.);caption(ui,"RESSOURCE VISA / SCPI");changed|=ui.text_edit_singleline(&mut c.resource).changed();
                        ui.label(RichText::new("SIM::RF::INSTR\nTCPIP::192.168.1.20::5025::SOCKET").monospace().size(10.).color(MUTED));
                    }
                    ui.add_space(12.);ui.separator();caption(ui,"CONNECTIQUE");
                    if let Some(p)=n.kind.input(){ui.label(format!("Entrée : {}",p.label()));}ui.label(format!("Sortie : {}",n.kind.output().label()));
                    ui.add_space(10.);if ui.button(RichText::new("Supprimer le bloc").color(RED)).clicked(){remove=Some(id);}
                }else{ui.label("Sélectionner un bloc dans le schéma.");}
                if changed && let Some(old)=previous_node {
                    let mut before=self.project.graph.clone();
                    if let Some(n)=before.nodes.iter_mut().find(|n|n.id==old.id){*n=old;}
                    self.history.record(before);self.canvas.completed.clear();
                }
                if let Some(id)=remove{self.history.record(self.project.graph.clone());self.project.graph.remove(id);self.canvas.selected=None;}
                if open_python{self.view=View::Python;}
                ui.add_space(22.);ui.separator();ui.add_space(8.);caption(ui,"FICHIERS DU BANC");
                ui.label("Projet (.rfw.json)");ui.add(egui::TextEdit::singleline(&mut self.project_path).desired_width(f32::INFINITY));
                ui.label("Export de trace (.csv)");ui.add(egui::TextEdit::singleline(&mut self.csv_path).desired_width(f32::INFINITY));
                if ui.button("Exporter la trace").clicked(){self.export();}
            });
        });
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
        egui::TopBottomPanel::bottom("journal")
            .resizable(true)
            .default_height(126.)
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
                        let height = (ui.available_height() - 245.).max(270.);
                        if let Some(error) =
                            self.canvas
                                .show(ui, &mut self.project.graph, &mut self.history, height)
                        {
                            self.log(error, true);
                        }
                        ui.add_space(12.);
                        self.trace_header(ui);
                        plot::plot(ui, &self.trace, 190., self.marker);
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
        let wants_input = ctx.wants_keyboard_input();
        if !wants_input {
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) {
                self.history.undo(&mut self.project.graph);
            }
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Y)) {
                self.history.redo(&mut self.project.graph);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Delete))
                && let Some(id) = self.canvas.selected
            {
                self.history.record(self.project.graph.clone());
                self.project.graph.remove(id);
                self.canvas.selected = None;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::F5)) && !self.worker.is_busy() {
                self.run();
            }
        }
        self.top(ctx);
        self.bottom(ctx);
        self.sidebar(ctx);
        self.inspector(ctx);
        self.center(ctx);
        self.frame_ms = self.frame_ms * 0.9 + start.elapsed().as_secs_f64() * 1000. * 0.1;
        self.last_frame = Instant::now();
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
