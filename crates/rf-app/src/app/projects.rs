use super::*;
impl Workbench {
    fn create_bench(&mut self, graph: rf_core::Graph, name: &str) {
        self.snapshot_workspace();
        let project = Project {
            schema_version: rf_core::SCHEMA_VERSION,
            name: name.into(),
            graph,
        };
        self.studio
            .workspaces
            .push(Workspace::new(name.into(), project));
        self.studio.active = self.studio.workspaces.len() - 1;
        self.restore_workspace(self.studio.active);
        self.welcome = false;
        self.project_path = std::path::Path::new(&self.browse_directory)
            .join("new-bench.rfbench")
            .to_string_lossy()
            .into_owned();
    }

    pub(super) fn welcome_view(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx,|ui|{
            ui.add_space(45.);ui.heading("RF WORKBENCH");ui.label("Instrumentation RF · simulation · métrologie");ui.add_space(30.);
            ui.heading("Créer ou ouvrir un banc de mesure");
            if ui.button("Revenir au banc courant").clicked(){self.welcome=false;}
            ui.label("Un fichier .rfbench conserve le schéma, les réglages des blocs et la disposition des panneaux.");
            ui.add_space(20.);ui.label("Nom du nouveau projet");ui.add(egui::TextEdit::singleline(&mut self.workspace_name).desired_width(500.));
            ui.horizontal_wrapped(|ui|{
                if ui.add_sized([230.,55.],egui::Button::new("+ Nouveau projet vide")).clicked(){
                    self.create_bench(rf_core::Graph::default(),&self.workspace_name.clone());
                }
                if ui.add_sized([230.,55.],egui::Button::new("Ouvrir un projet…")).clicked(){self.project_dialog=Some(false);}
            });
            ui.add_space(25.);ui.separator();ui.heading("Partir d'un exemple");
            ui.horizontal_wrapped(|ui|{
                for (label,index) in [("PNA-X / paramètres S",0),("RF / DSP / QAM16",1),("PA 36–38 GHz · 20 dB",2),("RF / spectre",3)] {
                    if ui.button(label).clicked(){let (graph,name)=match index {0=>(rf_core::Graph::network_demo(),"PNA-X / DUT"),1=>(rf_runtime::dsp_demo(),"RF / DSP / QAM16"),2=>(rf_core::Graph::pa_demo(),"PA 36–38 GHz · gain 20 dB (simulation)"),_=>(rf_core::Graph::demo(),"RF / spectre")};self.create_bench(graph,name);}
                }
            });
            ui.add_space(20.);ui.label("Simulation au démarrage. Les ressources instrument sont mémorisées ; le mode Matériel doit être activé à chaque ouverture.");
            ui.label("Fichier historique .rfw.json accepté à l'ouverture. Les nouveaux enregistrements utilisent .rfbench.");
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
                            .selectable_label(self.project_path == path.to_string_lossy(), label)
                            .clicked()
                        {
                            if dir {
                                self.browse_directory = path.to_string_lossy().into_owned();
                            } else {
                                self.project_path = path.to_string_lossy().into_owned();
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
            ui.add(egui::TextEdit::singleline(&mut self.project_path).desired_width(f32::INFINITY));
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
