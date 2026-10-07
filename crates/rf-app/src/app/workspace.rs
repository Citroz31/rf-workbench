use super::*;
impl Workbench {
    pub(super) fn network_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            caption(ui, "ANALYSE RÉSEAU / PNA & PNA-X");
            if ui.button("Charger la démo PNA-X").clicked() {
                self.replace_demo(
                    rf_core::Graph::network_demo(),
                    "Caractérisation réseau · PNA-X & DUT",
                );
            }
        });
        if let Some(t) = &self.network {
            badge(
                ui,
                &format!(
                    "{} · {} points · {}",
                    t.parameter,
                    t.frequency_hz.len(),
                    if t.simulated { "SIMULÉ" } else { "MATÉRIEL" }
                ),
                BLUE,
            );
            ui.label("Magnitude");
            plot::series(ui, &t.frequency_hz, &t.magnitude_db, 210., "GHz", "dB", 1e9);
            ui.add_space(8.);
            ui.label("Phase");
            plot::series(ui, &t.frequency_hz, &t.phase_deg, 160., "GHz", "°", 1e9);
        } else {
            ui.label("Aucun balayage réseau acquis. Charger la démo puis cliquer Exécuter.");
        }
        ui.label(RichText::new("Le modèle PNA illustre une perte et une phase idéales. Il ne représente ni une calibration VNA, ni des données constructeur.").size(12.).color(MUTED));
    }
    pub(super) fn measurement_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            caption(ui, "FORMES D'ONDE / MESURES SCALAIRES");
            if ui.button("Charger la démo I/Q").clicked() {
                self.replace_demo(rf_core::Graph::iq_demo(), "Chaîne I/Q · AWG & DAC");
            }
        });
        if let Some(w) = &self.waveform {
            badge(
                ui,
                &format!(
                    "{} échantillons · {:.1} MS/s · SIMULÉ",
                    w.samples.len(),
                    w.sample_rate_hz / 1e6
                ),
                PURPLE,
            );
            let time: Vec<_> = (0..w.samples.len())
                .map(|i| i as f64 / w.sample_rate_hz)
                .collect();
            plot::series(ui, &time, &w.samples, 240., "µs", &w.unit, 1e-6);
        } else {
            ui.label("Aucune forme d'onde. Exécuter une chaîne AWG / DAC / CAN.");
        }
        ui.add_space(15.);
        egui::Grid::new("scalar-results")
            .striped(true)
            .show(ui, |ui| {
                ui.strong("Bloc");
                ui.strong("Valeur");
                ui.strong("Unité");
                ui.strong("Origine");
                ui.end_row();
                for m in &self.measurements {
                    ui.label(&m.name);
                    ui.monospace(format!("{:.4}", m.value));
                    ui.colored_label(TEAL, &m.unit);
                    ui.label(if m.simulated {
                        "Simulée"
                    } else {
                        "Matérielle"
                    });
                    ui.end_row();
                }
            });
        if self.measurements.is_empty() {
            ui.label(RichText::new("Les températures, résistances, facteurs de bruit et puissances apparaîtront ici après exécution.").color(MUTED));
        }
    }
    fn replace_demo(&mut self, graph: rf_core::Graph, name: &str) {
        self.history.record(self.project.graph.clone());
        self.project.graph = graph;
        self.project.name = name.into();
        self.canvas = Canvas::configured(self.preferences.snap, self.preferences.orthogonal);
        self.view = View::Schematic;
        self.network = None;
        self.waveform = None;
        self.measurements.clear();
        self.preview = true;
        self.tests.clear();
    }
    pub(super) fn library_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            caption(ui, "INSTRUMENTS & SYMBOLES");
            ui.label(RichText::new("Clique un élément pour l'ajouter au schéma").color(MUTED));
        });
        let columns = (ui.available_width() / 200.).floor().max(1.) as usize;
        let width = (ui.available_width() - 10. * (columns - 1) as f32) / columns as f32;
        let mut add = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("visual-library")
                .spacing([10., 10.])
                .show(ui, |ui| {
                    for (i, k) in Kind::ALL
                        .into_iter()
                        .filter(|k| {
                            k.label()
                                .to_lowercase()
                                .contains(&self.search.to_lowercase())
                        })
                        .enumerate()
                    {
                        let (rect, response) =
                            ui.allocate_exact_size(egui::vec2(width, 165.), egui::Sense::click());
                        let p = ui.painter();
                        p.rect_filled(rect, 8., if response.hovered() { BORDER } else { CARD });
                        p.rect_stroke(
                            rect,
                            8.,
                            egui::Stroke::new(1., crate::theme::kind(k).gamma_multiply(0.5)),
                            egui::StrokeKind::Inside,
                        );
                        p.text(
                            rect.min + egui::vec2(12., 18.),
                            egui::Align2::LEFT_CENTER,
                            k.label(),
                            egui::FontId::proportional(14.),
                            TEXT,
                        );
                        visuals::symbol(
                            p,
                            egui::Rect::from_min_size(
                                rect.min + egui::vec2(18., 38.),
                                egui::vec2(width - 36., 78.),
                            ),
                            k,
                        );
                        p.text(
                            rect.min + egui::vec2(12., 132.),
                            egui::Align2::LEFT_CENTER,
                            k.tag(),
                            egui::FontId::proportional(10.),
                            crate::theme::kind(k),
                        );
                        p.text(
                            rect.min + egui::vec2(12., 150.),
                            egui::Align2::LEFT_CENTER,
                            format!(
                                "{} IN / {} OUT · Ajouter",
                                k.inputs().len(),
                                k.outputs().len()
                            ),
                            egui::FontId::proportional(10.),
                            MUTED,
                        );
                        if response.clicked() {
                            add = Some(k);
                        }
                        if (i + 1) % columns == 0 {
                            ui.end_row();
                        }
                    }
                });
        });
        if let Some(k) = add {
            self.add_block(k);
        }
    }
    pub(super) fn settings_view(&mut self, ui: &mut egui::Ui) {
        caption(ui, "RACCOURCIS PERSONNALISABLES");
        ui.label("Plusieurs touches par action, séparées par des virgules : W, Ctrl+Shift+W. Ctrl correspond à Cmd sur macOS. Les raccourcis sont suspendus pendant la saisie de texte.");
        ui.horizontal(|ui| {
            if ui.button("Enregistrer les préférences").clicked() {
                match self
                    .preferences_draft
                    .validate()
                    .and_then(|_| {
                        serde_json::to_string_pretty(&self.preferences_draft)
                            .map_err(|e| e.to_string())
                    })
                    .and_then(|s| atomic_save(&self.preferences_path, &s))
                {
                    Ok(()) => {
                        self.preferences = self.preferences_draft.clone();
                        self.canvas.snap = self.preferences.snap;
                        self.canvas.orthogonal = self.preferences.orthogonal;
                        self.log("Raccourcis enregistrés et actifs".into(), false);
                    }
                    Err(e) => self.log(e, true),
                }
            }
            if ui.button("Restaurer les valeurs par défaut").clicked() {
                self.preferences_draft = Preferences::default();
            }
        });
        if let Err(e) = self.preferences_draft.validate() {
            ui.colored_label(RED, e);
        }
        ui.label(
            RichText::new(&self.preferences_path)
                .monospace()
                .size(10.)
                .color(MUTED),
        );
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.preferences_draft.snap, "Aimantation à la grille");
            ui.checkbox(
                &mut self.preferences_draft.orthogonal,
                "Parcours orthogonal",
            );
            ui.checkbox(&mut self.preferences_draft.show_journal, "Journal visible");
        });
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("shortcuts")
                .striped(true)
                .min_col_width(250.)
                .show(ui, |ui| {
                    for b in &mut self.preferences_draft.shortcuts {
                        ui.label(b.action.label());
                        ui.add(egui::TextEdit::singleline(&mut b.keys).desired_width(300.));
                        ui.end_row();
                    }
                });
        });
    }
    pub(super) fn catalog_view(&mut self, ui: &mut egui::Ui) {
        caption(ui, "BIBLIOTHÈQUE DUT / FICHES LOCALES");
        ui.label(RichText::new("Le catalogue est volontairement vide à l'origine. Les futurs imports constructeur devront conserver les URL, dates de récupération et unités.").color(MUTED));
        ui.label(format!(
            "Fabricants prévus : {}",
            self.catalog.manufacturers.join(", ")
        ));
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.catalog_path).desired_width(500.));
            if ui.button("Importer JSON").clicked() {
                match read_limited(&self.catalog_path, 4_000_000)
                    .and_then(|s| Catalog::from_json(&s).map_err(|e| e.to_string()))
                {
                    Ok(c) => {
                        self.catalog = c;
                        self.log("Catalogue DUT importé".into(), false);
                    }
                    Err(e) => self.log(e, true),
                }
            }
            if ui.button("Enregistrer").clicked() {
                match self
                    .catalog
                    .to_json()
                    .map_err(|e| e.to_string())
                    .and_then(|s| atomic_save(&self.catalog_path, &s))
                {
                    Ok(()) => self.log("Catalogue DUT enregistré".into(), false),
                    Err(e) => self.log(e, true),
                }
            }
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.catalog_search)
                .hint_text("Fabricant, référence, famille…")
                .desired_width(450.),
        );
        ui.add_space(10.);
        let mut chosen = None;
        egui::ScrollArea::vertical()
            .max_height(180.)
            .show(ui, |ui| {
                for c in self.catalog.search(&self.catalog_search) {
                    ui.horizontal(|ui| {
                        ui.strong(format!("{} / {}", c.manufacturer, c.part_number));
                        ui.label(&c.category);
                        ui.label(if c.sources.is_empty() {
                            "Fiche locale"
                        } else {
                            "Source documentée"
                        });
                        if ui.button("Créer un bloc DUT").clicked() {
                            chosen = Some(c.clone());
                        }
                    });
                }
            });
        if let Some(c) = chosen {
            self.add_block(Kind::Dut);
            if let Some(n) = self.project.graph.nodes.last_mut() {
                n.title = format!("{} {}", c.manufacturer, c.part_number);
                n.config.dut_id = Some(c.id);
                n.config.loss_db = c.insertion_loss_db.unwrap_or(0.);
                n.config.noise_figure_db = c.noise_figure_db.unwrap_or(0.);
            }
        }
        ui.separator();
        caption(ui, "AJOUTER UNE FICHE MANUELLE");
        egui::Grid::new("component-draft").show(ui, |ui| {
            ui.label("Identifiant unique");
            ui.text_edit_singleline(&mut self.component_draft.id);
            ui.end_row();
            ui.label("Fabricant");
            ui.text_edit_singleline(&mut self.component_draft.manufacturer);
            ui.end_row();
            ui.label("Référence");
            ui.text_edit_singleline(&mut self.component_draft.part_number);
            ui.end_row();
            ui.label("Famille");
            ui.text_edit_singleline(&mut self.component_draft.category);
            ui.end_row();
            ui.label("Description");
            ui.text_edit_singleline(&mut self.component_draft.description);
            ui.end_row();
            ui.label("Perte simulée");
            ui.add(
                egui::DragValue::new(self.component_draft.insertion_loss_db.get_or_insert(0.))
                    .range(0. ..=160.)
                    .suffix(" dB"),
            );
            ui.end_row();
            ui.label("NF simulé");
            ui.add(
                egui::DragValue::new(self.component_draft.noise_figure_db.get_or_insert(0.))
                    .range(0. ..=60.)
                    .suffix(" dB"),
            );
            ui.end_row();
        });
        if ui.button("Ajouter au catalogue local").clicked() {
            let mut next = self.catalog.clone();
            next.entries.push(self.component_draft.clone());
            match next.validate() {
                Ok(()) => {
                    self.catalog = next;
                    self.log(
                        "Fiche locale ajoutée ; enregistrer le catalogue pour la conserver".into(),
                        false,
                    );
                }
                Err(e) => self.log(e.to_string(), true),
            }
        }
    }
    pub(super) fn project_view(&mut self, ui: &mut egui::Ui) {
        caption(ui, "PROJET / FICHIERS");
        ui.label("Nom du banc");
        ui.text_edit_singleline(&mut self.project.name);
        ui.label("Projet (.rfw.json)");
        ui.add(egui::TextEdit::singleline(&mut self.project_path).desired_width(f32::INFINITY));
        ui.horizontal(|ui| {
            if ui.button("Enregistrer le projet").clicked() {
                self.action(Action::Save);
            }
            if ui.button("Ouvrir le projet").clicked() {
                self.action(Action::Open);
            }
        });
        ui.label("Export spectre (.csv)");
        ui.add(egui::TextEdit::singleline(&mut self.csv_path).desired_width(f32::INFINITY));
        if ui.button("Exporter le spectre courant").clicked() {
            self.export();
        }
        ui.add_space(20.);
        caption(ui, "BANCS DE DÉMONSTRATION");
        ui.horizontal_wrapped(|ui| {
            if ui.button("RF / spectre").clicked() {
                self.replace_demo(rf_core::Graph::demo(), "Chaîne RF · 2.45 GHz");
            }
            if ui.button("PNA-X / DUT").clicked() {
                self.replace_demo(
                    rf_core::Graph::network_demo(),
                    "Caractérisation réseau · PNA-X & DUT",
                );
            }
            if ui.button("AWG / DAC / I/Q").clicked() {
                self.replace_demo(rf_core::Graph::iq_demo(), "Chaîne I/Q · AWG & DAC");
            }
            if ui.button("Thermique / puissance").clicked() {
                self.replace_demo(
                    rf_runtime::measurement_demo(),
                    "Mesures thermiques & puissance RF",
                );
            }
        });
        ui.add_space(15.);
        ui.label(format!(
            "{} blocs / {} câbles",
            self.project.graph.nodes.len(),
            self.project.graph.edges.len()
        ));
        match self.project.graph.validate() {
            Ok(()) => {
                ui.colored_label(TEAL, "Schéma exécutable");
            }
            Err(e) => {
                ui.colored_label(GOLD, e.to_string());
            }
        }
    }
}
