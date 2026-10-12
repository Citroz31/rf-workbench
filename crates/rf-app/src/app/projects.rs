use super::*;
impl Workbench {
    pub(super) fn create_bench(&mut self, graph: rf_core::Graph, suggested: &str) {
        if self.worker.is_busy() || self.pending_project.is_some() {
            return;
        }
        self.new_project = Some(crate::new_project::Draft::new(
            graph,
            suggested,
            self.browse_directory.clone(),
        ));
    }
    pub(super) fn new_project_window(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.new_project.take() else {
            return;
        };
        let mut open = true;
        let mut create = false;
        let mut cancel = false;
        egui::Window::new("Créer un nouveau projet")
            .id(egui::Id::new("new-project"))
            .open(&mut open)
            .resizable(true)
            .collapsible(false)
            .default_width(540.)
            .anchor(egui::Align2::CENTER_CENTER, [0., 0.])
            .show(ctx, |ui| {
                ui.label("Nom du projet *");
                let name = ui.add(
                    egui::TextEdit::singleline(&mut draft.name)
                        .id_salt("new-project-name")
                        .hint_text("Ex. Caractérisation PA bande Ka")
                        .char_limit(128)
                        .desired_width(f32::INFINITY),
                );
                if draft.focus_name { name.request_focus(); draft.focus_name=false; }
                if !draft.suggested_name.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.small("Suggestion :");
                        if ui.small_button(&draft.suggested_name).clicked() {
                            draft.name = draft.suggested_name.replace(['/', '\\'], " - ");
                        }
                    });
                }
                ui.add_space(8.);
                ui.label("Dossier d'enregistrement *");
                ui.add(
                    egui::TextEdit::singleline(&mut draft.directory).desired_width(f32::INFINITY),
                );
                ui.horizontal(|ui| {
                    if ui.button("Choisir un dossier…").clicked() {
                        draft.folders = !draft.folders;
                    }
                    if ui.button("Dossier local par défaut").clicked() {
                        draft.directory = self.data_directory.path().display().to_string();
                    }
                });
                if draft.folders {
                    ui.horizontal(|ui| {
                        if ui.button("Dossier parent").clicked()
                            && let Some(parent) = std::path::Path::new(&draft.directory).parent()
                        {
                            draft.directory = parent.display().to_string();
                        }
                    });
                    if self.file_browser.update(&draft.directory) {
                        ctx.request_repaint_after(Duration::from_millis(50));
                    }
                    egui::ScrollArea::vertical()
                        .max_height(170.)
                        .show(ui, |ui| {
                            if let Some(e) = &self.file_browser.error {
                                ui.colored_label(red(), e);
                            }
                            for (p, is_dir) in self.file_browser.entries.clone() {
                                if is_dir
                                    && ui
                                        .button(format!(
                                            "▸ {}",
                                            p.file_name().unwrap_or_default().to_string_lossy()
                                        ))
                                        .clicked()
                                {
                                    draft.directory = p.display().to_string();
                                }
                            }
                        });
                }
                ui.separator();
                match draft.destination() {
                    Ok(path) => {
                        ui.small("Le fichier sera créé immédiatement :");
                        ui.monospace(path.display().to_string());
                    }
                    Err(e) => {
                        ui.small(e);
                    }
                }
                if !draft.error.is_empty() {
                    ui.colored_label(red(), &draft.error);
                }
                ui.horizontal(|ui| {
                    create = ui
                        .add_enabled(
                            draft.destination().is_ok() && !self.worker.is_busy(),
                            egui::Button::new("Créer et enregistrer"),
                        )
                        .clicked();
                    cancel = ui.button("Annuler").clicked();
                });
                if name.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    create = draft.destination().is_ok();
                }
            });
        if cancel || !open {
            return;
        }
        if create {
            match self.recover_project().and_then(|_| draft.create()) {
                Ok((file, path)) => {
                    self.browse_directory = draft.directory;
                    self.request_project(file, path, true);
                    return;
                }
                Err(e) => draft.error = e,
            }
        }
        self.new_project = Some(draft);
    }
    pub(super) fn remember_project(&mut self) {
        self.registry
            .remember(&self.project_path, &self.project.name);
        if let Err(e) = serde_json::to_string_pretty(&self.registry)
            .map_err(|e| e.to_string())
            .and_then(|s| atomic_save(&self.registry_path, &s))
        {
            self.log(e, true);
        }
    }
    pub(super) fn recover_project(&mut self) -> Result<(), String> {
        if !self.active_project {
            return Ok(());
        }
        let text =
            crate::bench_file::BenchFile::new(self.project.clone(), self.layout.clone()).json()?;
        if text == self.last_saved {
            return Ok(());
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = self
            .data_directory
            .file(&format!("recovery-{stamp}.rfbench"));
        atomic_save(&path, &text)?;
        self.registry.remember(
            &path,
            &format!(
                "Récupération · {}",
                self.project.name.chars().take(40).collect::<String>()
            ),
        );
        if let Ok(text) = serde_json::to_string_pretty(&self.registry) {
            atomic_save(&self.registry_path, &text)?;
        }
        self.log(
            format!("Modifications non enregistrées récupérables : {path}"),
            false,
        );
        Ok(())
    }
    pub(super) fn request_project(
        &mut self,
        file: crate::bench_file::BenchFile,
        path: String,
        is_new: bool,
    ) {
        if self.worker.is_busy() || self.pending_project.is_some() {
            return;
        }
        if !is_new && let Err(e) = self.recover_project() {
            self.log(
                format!("Sauvegarder le projet avant de changer : {e}"),
                true,
            );
            return;
        }
        self.pending_project = Some((file, path, is_new));
        if let Err(e) = self.worker.submit(Command::ResetProject) {
            self.pending_project = None;
            self.log(e, true);
        }
    }
    pub(super) fn install_project(
        &mut self,
        file: crate::bench_file::BenchFile,
        path: String,
        is_new: bool,
    ) {
        self.project = file.project;
        self.layout = file.layout;
        self.view = self.layout.primary;
        self.project_path = path.clone();
        self.dialog_path = path;
        self.studio.workspaces = vec![Workspace::new(
            self.project.name.clone(),
            self.project.clone(),
        )];
        self.studio.active = 0;
        self.hardware = false;
        self.continuous = false;
        self.history.clear();
        self.canvas = Canvas::configured(self.preferences.snap, self.preferences.orthogonal);
        self.routing_job = None;
        self.clipboard = None;
        self.instrument_dialog = None;
        self.setup_rename = None;
        self.project_dialog = None;
        self.new_project = None;
        self.network = None;
        self.waveform = None;
        self.buffers.clear();
        self.curves.clear();
        self.waterfall = Waterfall::default();
        self.debug_snapshot = None;
        self.resources.clear();
        self.resource = "SIM::RF::INSTR".into();
        self.query = "*IDN?".into();
        self.script = rf_core::Config::default().script;
        self.trace = rf_instruments::simulate_trace(&rf_core::Config::default(), 2.45e9, -13., 0)
            .expect("fresh preview");
        self.result_selection = 0;
        self.result_target = None;
        self.clock = Instant::now();
        self.measurements.clear();
        self.tests.clear();
        self.dsp_history = dsp::History::default();
        self.dsp_draft = dsp::Draft::default();
        self.dsp_iq = None;
        self.dsp_spectrum = None;
        self.iq_keys = [None, None];
        self.wave_key = None;
        self.buffer_key = None;
        self.flow_report = None;
        self.results_open = false;
        self.fixture_job = Default::default();
        self.trace_display = TraceDisplay::Unavailable;
        self.preview = false;
        self.sequence = 0;
        self.elapsed = 0.;
        self.csv_path = std::path::Path::new(&self.project_path)
            .with_extension("csv")
            .to_string_lossy()
            .into_owned();
        self.layout_revision += 1;
        self.logs.clear();
        self.active_project = true;
        self.welcome = false;
        self.last_saved = if is_new && !std::path::Path::new(&self.project_path).is_file() {
            String::new()
        } else {
            crate::bench_file::BenchFile::new(self.project.clone(), self.layout.clone())
                .json()
                .unwrap_or_default()
        };
        if !is_new || std::path::Path::new(&self.project_path).is_file() {
            self.remember_project();
        }
        if is_new {
            self.canvas.fit();
        } else {
            self.canvas
                .restore_view(self.layout.canvas_pan, self.layout.canvas_zoom);
        }
        self.log(
            "Projet indépendant ouvert · aucun résultat ni autorisation Matériel restauré".into(),
            false,
        );
    }
    pub(super) fn welcome_view(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx,|ui|{
            ui.add_space(45.);ui.heading("RF WORKBENCH");ui.label("Instrumentation RF · simulation · métrologie");ui.add_space(30.);
            ui.heading("Créer ou ouvrir un banc de mesure");
            if self.active_project && ui.button("Revenir au projet courant").clicked(){self.welcome=false;}
            ui.label("Un fichier .rfbench conserve le schéma, les réglages des blocs et la disposition des panneaux.");
             ui.add_space(20.);
            ui.horizontal_wrapped(|ui|{
                if ui.add_sized([230.,55.],egui::Button::new("+ Nouveau projet vide")).clicked(){
                    self.create_bench(rf_core::Graph::default(),"Banc RF");
                }
                if ui.add_sized([230.,55.],egui::Button::new("Ouvrir un projet…")).clicked(){self.project_dialog=Some(false);}
            });
            ui.add_space(20.);ui.heading("Projets récents sur ce PC");
            let mut recent_path=None;
            for recent in self.registry.recent.iter().take(8){ui.horizontal(|ui|{if ui.button(&recent.name).clicked(){recent_path=Some(recent.path.clone());}ui.small(&recent.path);});}
            if let Some(path)=recent_path {self.dialog_path=path;self.load();}
            ui.add_space(25.);ui.separator();ui.heading("Partir d'un exemple");
            ui.horizontal_wrapped(|ui|{
                 for (label,index) in [("PNA-X / paramètres S",0),("RF / DSP / QAM16",1),("PA 36–38 GHz · 20 dB",2),("RF / spectre",3)] {
                     if ui.button(label).clicked(){let (graph,name)=match index {0=>(rf_core::Graph::network_demo(),"PNA-X / DUT"),1=>(rf_runtime::dsp_demo(),"RF / DSP / QAM16"),2=>(rf_core::Graph::pa_demo(),"PA 36–38 GHz · gain 20 dB (simulation)"),_=>(rf_core::Graph::demo(),"RF / spectre")};self.create_bench(graph,name);}
                 }
                 for (label,text) in [("LNA MAAL-FR1245",include_str!("../../../../examples/LNA-MAAL-FR1245.rfbench")),("Core chip CGY2170",include_str!("../../../../examples/Corechip-CGY2170YHV-C1.rfbench"))] {
                     if ui.button(label).clicked() {match crate::bench_file::BenchFile::parse(text) {
                         Ok(b)=>{self.create_bench(b.project.graph,&b.project.name);if let Some(draft)=&mut self.new_project {draft.layout=b.layout;}}
                         Err(e)=>self.log(e,true),
                     }}
                 }
            });
            ui.add_space(20.);ui.label("Simulation au démarrage. Les ressources instrument sont mémorisées ; le mode Matériel doit être activé à chaque ouverture.");
            ui.label("Fichier historique .rfw.json accepté à l'ouverture. Les nouveaux enregistrements utilisent .rfbench.");
            ui.add_space(12.);ui.label("Dossier local des projets et réglages :");
            ui.monospace(self.data_directory.path().display().to_string());
            if let Some(warning)=self.data_directory.warning(){ui.colored_label(red(),warning);}
        });
    }
    pub(super) fn project_window(&mut self, ctx: &egui::Context) {
        let Some(save) = self.project_dialog else {
            return;
        };
        let mut open = true;
        let mut act = false;
        egui::Window::new(if save {
            "Enregistrer le banc"
        } else {
            "Ouvrir un banc"
        })
        .id(egui::Id::new("project-browser"))
        .open(&mut open)
        .resizable(true)
        .default_size([730., 480.])
        .show(ctx, |ui| {
            ui.label("Dossier");
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.browse_directory)
                        .desired_width(ui.available_width() - 85.),
                );
                if ui.button("Parent").clicked()
                    && let Some(p) = std::path::Path::new(&self.browse_directory).parent()
                {
                    self.browse_directory = p.to_string_lossy().into_owned();
                }
            });
            if self.file_browser.update(&self.browse_directory) {
                ctx.request_repaint_after(Duration::from_millis(50));
                ui.label("Lecture du dossier…");
            }
            egui::ScrollArea::vertical()
                .max_height(260.)
                .show(ui, |ui| {
                    if let Some(error) = &self.file_browser.error {
                        ui.colored_label(red(), error);
                    }
                    for (path, dir) in self.file_browser.entries.clone() {
                        let label = format!(
                            "{} {}",
                            if dir { "▸" } else { "◇" },
                            path.file_name().unwrap_or_default().to_string_lossy()
                        );
                        if ui
                            .selectable_label(self.dialog_path == path.to_string_lossy(), label)
                            .clicked()
                        {
                            if dir {
                                self.browse_directory = path.to_string_lossy().into_owned();
                            } else {
                                self.dialog_path = path.to_string_lossy().into_owned();
                            }
                        }
                    }
                });
            ui.separator();
            ui.label(if save {
                "Chemin du fichier .rfbench"
            } else {
                "Chemin du fichier .rfbench ou .rfw.json"
            });
            ui.add(egui::TextEdit::singleline(&mut self.dialog_path).desired_width(f32::INFINITY));
            ui.horizontal(|ui| {
                act = ui
                    .add_enabled(
                        !self.worker.is_busy(),
                        egui::Button::new(if save { "Enregistrer" } else { "Ouvrir" }),
                    )
                    .clicked();
                if ui.button("Annuler").clicked() {
                    self.project_dialog = None;
                }
            });
        });
        if !open {
            self.project_dialog = None;
        }
        if act {
            if save {
                self.save();
                self.project_dialog = None;
            } else {
                self.load();
            }
        }
    }
}
