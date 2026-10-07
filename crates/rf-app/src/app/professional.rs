use super::*;
use crate::{
    analysis, editor, help,
    studio::{Language, SavedLayout},
};
use rf_runtime::debug::BufferData;
const CLIPBOARD_PREFIX: &str = "RF_WORKBENCH_GRAPH_V1\n";

impl Workbench {
    pub(super) fn palette_item(&mut self, ui: &mut egui::Ui, k: Kind) {
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 47.), egui::Sense::click());
        let p = ui.painter();
        p.rect_filled(
            rect,
            5.,
            if response.hovered() {
                border()
            } else {
                card_fill()
            },
        );
        visuals::symbol(
            p,
            egui::Rect::from_min_size(rect.min + egui::vec2(4., 5.), egui::vec2(53., 35.)),
            k,
        );
        p.text(
            rect.min + egui::vec2(62., 16.),
            egui::Align2::LEFT_CENTER,
            t(k.label()),
            crate::theme::font(11.),
            crate::theme::kind(k),
        );
        p.text(
            rect.min + egui::vec2(62., 33.),
            egui::Align2::LEFT_CENTER,
            if self.studio.favorites.contains(&k) {
                "★"
            } else {
                pair(
                    "Ajouter · clic droit : favori",
                    "Add · right click: favorite",
                )
            },
            crate::theme::font(9.),
            muted(),
        );
        if response.clicked() {
            self.add_block(k);
        }
        response.context_menu(|ui| {
            if ui
                .button(if self.studio.favorites.contains(&k) {
                    pair("Retirer des favoris", "Remove favorite")
                } else {
                    pair("Ajouter aux favoris", "Add favorite")
                })
                .clicked()
            {
                self.studio.favorite(k);
                ui.close();
            }
            if ui.button(t("Aide du bloc")).clicked() {
                self.help_kind = Some(k);
                self.view = View::Help;
                ui.close();
            }
        });
    }
    fn graph_visible(&self) -> bool {
        self.view == View::Schematic || self.layout.panes.iter().any(|p| p.view == View::Schematic)
    }
    pub(super) fn copy_selection(&mut self) {
        if !self.graph_visible() {
            return;
        }
        let clip = editor::selection(&self.project.graph, &self.canvas.selected_ids);
        if clip.nodes.is_empty() {
            return;
        }
        if let Ok(json) = serde_json::to_string(&clip) {
            self.context.copy_text(format!("{CLIPBOARD_PREFIX}{json}"));
            self.clipboard = Some(clip);
        }
    }
    pub(super) fn paste_text(&mut self, text: &str) {
        if !self.graph_visible() {
            return;
        }
        if text.len() > 4_000_000 {
            self.log("Copie trop volumineuse".into(), true);
            return;
        }
        let result = text
            .strip_prefix(CLIPBOARD_PREFIX)
            .ok_or("Le presse-papiers ne contient pas de blocs RF Workbench".to_owned())
            .and_then(|json| {
                serde_json::from_str::<rf_core::Graph>(json).map_err(|e| e.to_string())
            });
        match result {
            Ok(clip) => {
                self.paste_graph(&clip);
                self.clipboard = Some(clip);
            }
            Err(e) => self.log(e, true),
        }
    }
    pub(super) fn paste_graph(&mut self, clip: &rf_core::Graph) {
        let before = self.project.graph.clone();
        match editor::paste(&mut self.project.graph, clip, [36., 36.]) {
            Ok(ids) => {
                self.history.record(before);
                self.canvas.selected_ids = ids;
                self.canvas.selected = self.canvas.selected_ids.iter().next().copied();
                self.canvas.completed.clear();
                self.canvas.fit();
                self.view = View::Schematic;
            }
            Err(e) => self.log(e, true),
        }
    }
    pub(super) fn delete_selection(&mut self) {
        if !self.graph_visible() {
            return;
        }
        let before = self.project.graph.clone();
        if let Some(id) = self.canvas.annotation.take() {
            self.project.graph.annotations.retain(|a| a.id != id);
        }
        for id in self.canvas.selected_ids.clone() {
            self.project.graph.remove(id);
        }
        if before != self.project.graph {
            self.history.record(before);
        }
        self.canvas.selected = None;
        self.canvas.selected_ids.clear();
        self.canvas.cancel_wire();
    }
    pub(super) fn organize(&mut self) {
        self.start_routing(true, false, true);
    }
    pub(super) fn route_all(&mut self) {
        self.start_routing(false, false, true);
    }
    pub(super) fn start_routing(&mut self, organize: bool, only_auto: bool, record: bool) {
        if self.routing_job.is_some() {
            self.log(
                pair("Routage déjà en cours", "Routing is already running").into(),
                false,
            );
            return;
        }
        let ctx = self.context.clone();
        self.routing_job = Some(editor::RoutingJob::spawn(
            &self.project.graph,
            organize,
            only_auto,
            record,
            move || ctx.request_repaint(),
        ));
        self.log(
            pair(
                "Recherche des parcours en arrière-plan…",
                "Computing paths in the background…",
            )
            .into(),
            false,
        );
    }
    pub(super) fn poll_routing(&mut self) {
        let Some(job) = self.routing_job.as_ref() else {
            return;
        };
        let report = match job.receiver.try_recv() {
            Ok(r) => r,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(e) => Err(e.to_string()),
        };
        let job = self.routing_job.take().unwrap();
        if self.project.graph != job.before {
            self.log(
                pair(
                    "Graphe modifié pendant le calcul : parcours ignorés ; relancer le routage.",
                    "Graph changed during routing: paths discarded; run routing again.",
                )
                .into(),
                false,
            );
            return;
        }
        match report {
            Ok(report) => {
                if report.graph != self.project.graph {
                    if job.record {
                        self.history.record(job.before);
                    }
                    self.project.graph = report.graph;
                }
                if job.record {
                    self.canvas.fit();
                    self.view = View::Schematic;
                }
                if report.failed > 0 {
                    self.log(
                        format!(
                            "{} {}",
                            report.failed,
                            pair(
                                "parcours non trouvés ; déplacer les blocs ou choisir des coudes.",
                                "paths not found; move blocks or choose manual bends."
                            )
                        ),
                        true,
                    );
                }
            }
            Err(e) => self.log(e, true),
        }
    }
    pub(super) fn add_annotation(&mut self) {
        if self.project.graph.annotations.len() >= 1000 {
            return;
        }
        self.history.record(self.project.graph.clone());
        let id = self
            .project
            .graph
            .annotations
            .iter()
            .map(|a| a.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let position = self
            .canvas
            .selected
            .and_then(|id| self.project.graph.node(id))
            .map(|n| [n.position[0], n.position[1] - 115.])
            .unwrap_or([36., -80.]);
        self.project.graph.annotations.push(rf_core::Annotation {
            id,
            text: pair(
                "Annotation — éditer dans l'inspecteur",
                "Annotation — edit in inspector",
            )
            .into(),
            position,
        });
        self.canvas.selected = None;
        self.canvas.selected_ids.clear();
        self.canvas.annotation = Some(id);
        self.view = View::Schematic;
    }
    pub(super) fn snapshot_workspace(&mut self) {
        if let Some(w) = self.studio.workspaces.get_mut(self.studio.active) {
            w.project = self.project.clone();
            w.project_path = self.project_path.clone();
            w.csv_path = self.csv_path.clone();
            w.layout = self.layout.clone();
            if self.view.dockable() {
                w.layout.primary = self.view;
            }
            w.pan = [self.canvas.pan.x, self.canvas.pan.y];
            w.zoom = self.canvas.zoom;
            w.history = self.history.clone();
            w.buffers = self.buffers.clone();
            w.trace = (self.trace_display == TraceDisplay::Acquired).then(|| self.trace.clone());
            w.network = self.network.clone();
            w.waveform = self.waveform.clone();
            w.measurements = self.measurements.clone();
            w.waterfall = self.waterfall.clone();
        }
    }
    pub(super) fn restore_workspace(&mut self, index: usize) {
        self.instrument_dialog = None;
        self.hardware = false;
        if self.worker.is_busy() {
            return;
        }
        self.routing_job = None;
        let Some(w) = self.studio.workspaces.get(index).cloned() else {
            return;
        };
        self.studio.active = index;
        self.project = w.project;
        if !w.project_path.is_empty() {
            self.project_path = w.project_path;
        }
        if !w.csv_path.is_empty() {
            self.csv_path = w.csv_path;
        }
        self.layout = w.layout;
        self.view = self.layout.primary;
        self.history = w.history;
        self.canvas = Canvas::configured(self.preferences.snap, self.preferences.orthogonal);
        self.canvas.restore_view(w.pan, w.zoom);
        self.dsp_history = super::dsp::History::default();
        self.dsp_draft = super::dsp::Draft::default();
        self.buffers = w.buffers;
        self.network = w.network;
        self.waveform = w.waveform;
        self.measurements = w.measurements;
        self.waterfall = w.waterfall;
        self.preview = w.trace.is_none()
            && self.network.is_none()
            && self.waveform.is_none()
            && self.measurements.is_empty();
        self.trace_display = if w.trace.is_some() {
            TraceDisplay::Acquired
        } else if self.preview {
            TraceDisplay::Preview
        } else {
            TraceDisplay::Unavailable
        };
        self.trace = w.trace.unwrap_or_else(|| {
            rf_instruments::simulate_trace(&rf_core::Config::default(), 2.45e9, -13., 0).unwrap()
        });
        self.tests.clear();
        self.debug_snapshot = None;
        self.iq_keys = [None, None];
        self.wave_key = None;
        self.buffer_key = None;
        self.layout_revision += 1;
    }
    pub(super) fn workspace_tabs(&mut self, ui: &mut egui::Ui) {
        let mut switch = None;
        for (i, w) in self.studio.workspaces.iter().enumerate() {
            if ui
                .add_enabled(
                    !self.worker.is_busy(),
                    egui::Button::new(format!("{} · {}", i + 1, w.name))
                        .selected(i == self.studio.active),
                )
                .clicked()
            {
                switch = Some(i);
            }
        }
        if let Some(i) = switch {
            self.snapshot_workspace();
            self.restore_workspace(i);
        }
    }
    fn save_studio(&mut self) {
        self.snapshot_workspace();
        match self
            .studio
            .validate()
            .and_then(|_| serde_json::to_string_pretty(&self.studio).map_err(|e| e.to_string()))
            .and_then(|s| atomic_save(&self.studio_path, &s))
        {
            Ok(()) => self.log(
                pair(
                    "Studio enregistré : espaces, layouts et préférences",
                    "Studio saved: workspaces, layouts and preferences",
                )
                .into(),
                false,
            ),
            Err(e) => self.log(e, true),
        }
    }
    pub(super) fn studio_view(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("studio-settings").show(ui,|ui| {
            ui.heading(t("Espaces de travail"));
            ui.label(pair("Chaque banc conserve son graphe, sa disposition, ses résultats et son historique de session.","Each bench keeps its graph, layout, results and session history."));
            ui.horizontal(|ui| {if ui.button(t("Enregistrer Studio")).clicked(){self.save_studio();}ui.monospace(&self.studio_path);});
            ui.separator();
            if let Some(w)=self.studio.workspaces.get_mut(self.studio.active) {ui.label(pair("Nom de l'espace actif","Active workspace name"));ui.add(egui::TextEdit::singleline(&mut w.name).char_limit(80));}
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.workspace_name).char_limit(80));
                if ui.add_enabled(!self.worker.is_busy()&&self.studio.workspaces.len()<8&&!self.workspace_name.trim().is_empty(),egui::Button::new(t("Nouvel espace"))).clicked() {
                    self.snapshot_workspace();
                    let project=Project{name:self.workspace_name.clone(),graph:rf_core::Graph::default(),..Project::default()};
                    let mut workspace=Workspace::new(self.workspace_name.clone(),project);
                    let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
                    let directory=std::path::Path::new(&self.project_path).parent().unwrap_or_else(||std::path::Path::new("."));
                    workspace.project_path=directory.join(format!("bench-{stamp}.rfw.json")).to_string_lossy().into_owned();
                    workspace.csv_path=directory.join(format!("trace-{stamp}.csv")).to_string_lossy().into_owned();
                    self.studio.workspaces.push(workspace);
                    self.restore_workspace(self.studio.workspaces.len()-1);self.canvas.fit();self.view=View::Studio;
                }
                if ui.add_enabled(!self.worker.is_busy()&&self.studio.workspaces.len()>1,egui::Button::new(pair("Fermer cet espace","Close workspace"))).on_hover_text(pair("Retire cet espace de Studio ; enregistrer vos projets avant de fermer.","Removes this workspace from Studio; save your projects before closing.")).clicked() {
                    self.studio.workspaces.remove(self.studio.active);self.studio.active=0;self.restore_workspace(0);self.view=View::Studio;
                }
            });
            ui.separator();ui.heading(t("Disposition"));
            ui.label(pair("Les menus Disposition de chaque vue permettent de la déplacer. Les fenêtres flottantes se déplacent par leur barre de titre.","Use each pane's Layout menu to move it. Floating windows move by their title bars."));
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.layout_name).char_limit(80));
                if ui.add_enabled(!self.layout_name.trim().is_empty()&&self.studio.layouts.len()<20,egui::Button::new(t("Enregistrer ce layout"))).clicked() {
                    let name=self.layout_name.trim().to_owned();self.studio.layouts.retain(|l|l.name!=name);
                    let layout=self.layout.clone();
                    self.studio.layouts.push(SavedLayout{name,layout});
                }
            });
            let mut load=None;let mut delete=None;
            for (i,l) in self.studio.layouts.iter().enumerate() {ui.horizontal(|ui| {ui.label(&l.name);if ui.small_button(t("Charger")).clicked(){load=Some(i);}
if ui.small_button(crate::i18n::t("×")).on_hover_text(pair("Retirer le layout","Remove layout")).clicked(){delete=Some(i);}});}
            if let Some(i)=load {self.layout=self.studio.layouts[i].layout.clone();self.view=self.layout.primary;self.layout_revision+=1;self.canvas.fit();}
            if let Some(i)=delete {self.studio.layouts.remove(i);}
            ui.horizontal(|ui| {
                if ui.button(pair("Layout banc + spectre","Bench + spectrum layout")).clicked(){self.layout=Layout::default();self.view=View::Schematic;self.layout_revision+=1;self.canvas.fit();}
                if ui.button(pair("Layout debug","Debug layout")).clicked(){self.layout=Layout::default();self.layout.set(View::Debug,Location::Right);self.view=View::Schematic;self.studio.inspector=false;self.layout_revision+=1;self.canvas.fit();}
            });
            ui.separator();ui.heading(t("Accessibilité"));
            ui.horizontal(|ui| {ui.checkbox(&mut self.studio.light,t("Thème clair"));ui.checkbox(&mut self.studio.high_contrast,t("Contraste renforcé"));});
            ui.add(egui::Slider::new(&mut self.studio.text_scale,0.85 ..=1.5).text(t("Taille du texte")));
            ui.horizontal(|ui| {ui.label(t("Langue"));ui.selectable_value(&mut self.studio.language,Language::French,t("Français"));ui.selectable_value(&mut self.studio.language,Language::English,t("Anglais"));});
            ui.checkbox(&mut self.studio.auto_route,t("Routage intelligent"));ui.checkbox(&mut self.studio.inspector,t("Inspecteur"));
            ui.label(pair("Tab/Shift+Tab : blocs ; flèches : ports ; Entrée : câble. États et unités lisibles sans dépendre de la couleur. Enregistrer Studio conserve ces réglages.","Tab/Shift+Tab: blocks; arrows: ports; Enter: wiring. States and units remain readable without color. Save Studio preserves these settings."));
        });
    }
    pub(super) fn pane_header(&mut self, ui: &mut egui::Ui, view: View) {
        if !view.dockable() {
            return;
        }
        ui.horizontal(|ui| {
            ui.strong(view.label());
            ui.menu_button(t("Disposition"), |ui| {
                for (where_, label) in [
                    (Location::Main, "Vue principale"),
                    (Location::Right, "À droite"),
                    (Location::Bottom, "En bas"),
                    (Location::Floating, "Fenêtre flottante"),
                    (Location::Hidden, "Masquer"),
                ] {
                    if ui.button(t(label)).clicked() {
                        self.layout.set(view, where_);
                        self.layout_revision += 1;
                        if where_ == Location::Main {
                            self.view = view;
                        } else if self.view == view {
                            self.view = if view == View::Schematic {
                                View::Acquisition
                            } else {
                                View::Schematic
                            };
                        }
                        self.canvas.fit();
                        ui.close();
                    }
                }
            });
            if ui
                .small_button(crate::i18n::t("?"))
                .on_hover_text(t("Aide"))
                .clicked()
            {
                self.help_kind = None;
                self.view = View::Help;
            }
        });
        ui.separator();
    }
    fn dock_tabs(&mut self, ui: &mut egui::Ui, views: &[View], location: Location) -> View {
        let active = if location == Location::Right {
            &mut self.layout.right_active
        } else {
            &mut self.layout.bottom_active
        };
        if active.is_none_or(|v| !views.contains(&v)) {
            *active = Some(views[0]);
        }
        ui.horizontal_wrapped(|ui| {
            for v in views {
                if ui
                    .selectable_label(*active == Some(*v), v.label())
                    .clicked()
                {
                    *active = Some(*v);
                }
            }
        });
        active.unwrap()
    }
    pub(super) fn docked(&mut self, ctx: &egui::Context) {
        if !self.view.dockable() {
            return;
        }
        let right: Vec<_> = self
            .layout
            .panes
            .iter()
            .filter(|p| p.location == Location::Right && p.view != self.view)
            .map(|p| p.view)
            .collect();
        if !right.is_empty() {
            let shown =
                egui::SidePanel::right(egui::Id::new(("analysis-right", self.layout_revision)))
                    .resizable(true)
                    .default_width(self.layout.right_width)
                    .width_range(240. ..=900.)
                    .show(ctx, |ui| {
                        let view = self.dock_tabs(ui, &right, Location::Right);
                        self.pane_header(ui, view);
                        if view == View::Debug {
                            self.render_view(ui, view);
                        } else {
                            egui::ScrollArea::vertical()
                                .id_salt(("right-content", view))
                                .show(ui, |ui| {
                                    self.render_view(ui, view);
                                });
                        }
                    });
            self.layout.right_width = shown.response.rect.width().clamp(240., 900.);
        }
        let bottom: Vec<_> = self
            .layout
            .panes
            .iter()
            .filter(|p| p.location == Location::Bottom && p.view != self.view)
            .map(|p| p.view)
            .collect();
        if !bottom.is_empty() {
            let shown = egui::TopBottomPanel::bottom(egui::Id::new((
                "analysis-bottom",
                self.layout_revision,
            )))
            .resizable(true)
            .default_height(self.layout.bottom_height)
            .height_range(130. ..=700.)
            .show(ctx, |ui| {
                let view = self.dock_tabs(ui, &bottom, Location::Bottom);
                self.pane_header(ui, view);
                if view == View::Acquisition {
                    self.trace_header(ui);
                    if self.trace_display == TraceDisplay::Unavailable {
                        ui.label(pair(
                            "Aucun spectre acquis par ce banc.",
                            "This bench has no acquired spectrum.",
                        ));
                    } else {
                        plot::plot(
                            ui,
                            &self.trace,
                            (self.layout.bottom_height - 145.).max(75.),
                            self.marker,
                        );
                    }
                } else if view == View::Schematic {
                    self.render_view(ui, view);
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt(("bottom-content", view))
                        .max_height((self.layout.bottom_height - 110.).max(60.))
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.render_view(ui, view));
                }
            });
            self.layout.bottom_height = shown.response.rect.height().clamp(130., 700.);
        }
    }
    pub(super) fn floating(&mut self, ctx: &egui::Context) {
        if !self.view.dockable() {
            return;
        }
        let panes: Vec<_> = self
            .layout
            .panes
            .iter()
            .filter(|p| p.location == Location::Floating && p.view != self.view)
            .cloned()
            .collect();
        for pane in panes {
            let mut open = true;
            let shown = egui::Window::new(pane.view.label())
                .id(egui::Id::new((
                    "floating-pane",
                    pane.view,
                    self.layout_revision,
                )))
                .open(&mut open)
                .default_pos([pane.rect[0], pane.rect[1]])
                .default_size([pane.rect[2], pane.rect[3]])
                .resizable(true)
                .min_size([300., 220.])
                .show(ctx, |ui| {
                    self.pane_header(ui, pane.view);
                    egui::ScrollArea::vertical()
                        .id_salt(("floating-content", pane.view))
                        .show(ui, |ui| {
                            self.render_view(ui, pane.view);
                        });
                });
            if !open {
                self.layout.set(pane.view, Location::Hidden);
            }
            if let Some(shown) = shown
                && let Some(p) = self.layout.panes.iter_mut().find(|p| p.view == pane.view)
            {
                let r = shown.response.rect;
                p.rect = [r.left(), r.top(), r.width().max(100.), r.height().max(100.)];
            }
        }
    }
    fn wave_selector(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        key_index: Option<usize>,
    ) -> Option<rf_core::Waveform> {
        let options: Vec<_> = self
            .buffers
            .iter()
            .filter_map(|b| {
                if let BufferData::Waveform(w) = b.data.as_ref() {
                    Some((
                        (b.node, b.port),
                        format!(
                            "#{} {} · {} · {}/{} pts",
                            b.node,
                            b.name,
                            w.unit,
                            w.samples.len(),
                            b.total_samples
                        ),
                    ))
                } else {
                    None
                }
            })
            .collect();
        let key = if let Some(i) = key_index {
            &mut self.iq_keys[i]
        } else {
            &mut self.wave_key
        };
        if key.is_none_or(|k| !options.iter().any(|(id, _)| *id == k)) {
            *key = options
                .get(key_index.unwrap_or(0).min(options.len().saturating_sub(1)))
                .map(|(id, _)| *id);
        }
        egui::ComboBox::from_id_salt(("wave-selector", label))
            .width(ui.available_width().min(500.))
            .selected_text(
                options
                    .iter()
                    .find(|(id, _)| Some(*id) == *key)
                    .map(|(_, name)| name.as_str())
                    .unwrap_or("—"),
            )
            .show_ui(ui, |ui| {
                for (id, name) in &options {
                    ui.selectable_value(key, Some(*id), name);
                }
            });
        key.and_then(|k| self.buffers.iter().find(|b| (b.node, b.port) == k))
            .and_then(|b| {
                if let BufferData::Waveform(w) = b.data.as_ref() {
                    Some(w.clone())
                } else {
                    None
                }
            })
    }
    pub(super) fn analysis_view(&mut self, ui: &mut egui::Ui, view: View) {
        match view {
            View::Waterfall => self.waterfall.show(ui),
            View::Smith => {
                ui.add(
                    egui::DragValue::new(&mut self.z0)
                        .range(1. ..=10000.)
                        .suffix(" Ω · Z₀"),
                );
                if let Some(t) = &self.network {
                    analysis::smith(ui, t, self.z0);
                } else {
                    ui.label(pair(
                        "Exécuter un PNA avec S11 ou S22 pour afficher le Smith.",
                        "Run a PNA with S11 or S22 to display Smith.",
                    ));
                }
            }
            View::Constellation => {
                caption(ui, "I");
                let i = self.wave_selector(ui, "I", Some(0));
                caption(ui, "Q");
                let q = self.wave_selector(ui, "Q", Some(1));
                ui.checkbox(
                    &mut self.trajectory,
                    pair(
                        "Tous les échantillons : trajectoire",
                        "All samples: trajectory",
                    ),
                );
                if !self.trajectory {
                    self.symbol_controls(ui);
                }
                ui.label(pair(
                    "Aucun synchroniseur ni EVM ; buffers acquis, même unité et cadence.",
                    "No synchronization or EVM; acquired buffers with matching units and rate.",
                ));
                if let (Some(i), Some(q)) = (i, q) {
                    analysis::constellation(
                        ui,
                        &i,
                        &q,
                        if self.trajectory {
                            1
                        } else {
                            self.samples_per_symbol
                        },
                        if self.trajectory {
                            0
                        } else {
                            self.sample_offset
                        },
                    );
                }
            }
            View::Eye => {
                let w = self.wave_selector(ui, "eye", None);
                self.symbol_controls(ui);
                ui.label(pair(
                    "Repliement 2 UI selon vos réglages ; pas de récupération d'horloge.",
                    "2 UI folding at your settings; no clock recovery.",
                ));
                if let Some(w) = w {
                    analysis::eye(ui, &w, self.samples_per_symbol, self.sample_offset);
                }
            }
            View::Timing => {
                let w = self.wave_selector(ui, "timing", None);
                ui.horizontal(|ui| {
                    ui.label(t("Seuil"));
                    ui.add(
                        egui::DragValue::new(&mut self.threshold)
                            .speed(0.01)
                            .range(-1e6..=1e6),
                    );
                    if let Some(w) = &w {
                        ui.label(&w.unit);
                    }
                });
                ui.label(pair(
                    "État 0/1 par seuillage ; aucun décodage de protocole.",
                    "Thresholded 0/1 state; no protocol decoding.",
                ));
                if let Some(w) = w {
                    analysis::timing(ui, &w, self.threshold);
                }
            }
            _ => {}
        }
    }
    fn symbol_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label(pair("Échantillons/UI", "Samples/UI"));
            ui.add(egui::DragValue::new(&mut self.samples_per_symbol).range(2..=256));
            self.sample_offset = self.sample_offset.min(self.samples_per_symbol - 1);
            ui.label(crate::i18n::t("Offset"));
            ui.add(
                egui::DragValue::new(&mut self.sample_offset)
                    .range(0..=self.samples_per_symbol - 1),
            );
        });
    }
    pub(super) fn debug_start(&mut self) {
        if self.worker.is_busy() {
            return;
        }
        self.buffers.clear();
        self.canvas.completed.clear();
        self.debug_snapshot = None;
        self.submit(Command::Debug {
            graph: self.project.graph.clone(),
            python_path: self.python_path.clone(),
        });
        self.layout.set(View::Debug, Location::Right);
        self.studio.inspector = false;
        self.view = View::Schematic;
        self.layout_revision += 1;
    }
    pub(super) fn debug_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !self.worker.is_busy(),
                    egui::Button::new(t("Démarrer le debug")),
                )
                .clicked()
            {
                self.debug_start();
            }
            let paused =
                self.debug_snapshot.as_ref().is_some_and(|s| s.paused) && self.worker.is_busy();
            if ui
                .add_enabled(paused, egui::Button::new(t("Pas à pas")))
                .clicked()
            {
                self.worker.debug_step();
            }
            if ui
                .add_enabled(paused, egui::Button::new(t("Continuer")))
                .clicked()
            {
                self.worker.debug_continue();
            }
            if ui
                .add_enabled(self.worker.is_busy(), egui::Button::new(t("Arrêter")))
                .clicked()
            {
                self.worker.stop();
            }
        });
        if let Some(s) = &self.debug_snapshot {
            ui.colored_label(
                gold(),
                format!(
                    "{} #{} {}",
                    if s.paused {
                        pair("Pause avant", "Paused before")
                    } else {
                        pair("Exécuté", "Executed")
                    },
                    s.node,
                    s.title
                ),
            );
        }
        ui.label(pair(
            "Instantané du banc au lancement · debug simulé par bloc.",
            "Bench snapshot at launch · simulated block-level debug.",
        ));
        ui.checkbox(&mut self.probes_only, t("Sondes uniquement"));
        let mut clicked = None;
        egui::ScrollArea::vertical()
            .id_salt("buffer-list")
            .max_height(170.)
            .show(ui, |ui| {
                for b in &self.buffers {
                    if self.probes_only && !self.project.graph.node(b.node).is_some_and(|n| n.probe)
                    {
                        continue;
                    }
                    if ui
                        .selectable_label(
                            self.buffer_key == Some((b.node, b.port)),
                            format!("#{} {} · {} pts", b.node, b.name, b.total_samples),
                        )
                        .clicked()
                    {
                        clicked = Some((b.node, b.port));
                    }
                }
            });
        if let Some(key) = clicked {
            self.buffer_key = Some(key);
        }
        if self.buffers.is_empty() {
            ui.label(pair(
                "Aucun buffer : exécuter ou avancer le debug.",
                "No buffer: run or step the debugger.",
            ));
            return;
        }
        if self
            .buffer_key
            .is_none_or(|key| !self.buffers.iter().any(|b| (b.node, b.port) == key))
        {
            self.buffer_key = self.buffers.first().map(|b| (b.node, b.port));
        }
        if let Some(b) = self
            .buffers
            .iter()
            .find(|b| Some((b.node, b.port)) == self.buffer_key)
        {
            ui.separator();
            ui.strong(&b.name);
            match b.data.as_ref() {
                BufferData::Dsp(d) => {
                    super::dsp::inspect(ui, d);
                }
                BufferData::Signal {
                    frequency_hz,
                    level_dbm,
                    simulated,
                } => {
                    ui.monospace(format!(
                        "{frequency_hz:.6} Hz\n{level_dbm:.6} dBm\nsimulated={simulated}"
                    ));
                }
                BufferData::Scalar {
                    value,
                    unit,
                    simulated,
                } => {
                    ui.monospace(format!("{value:.6} {unit}\nsimulated={simulated}"));
                }
                BufferData::Dut {
                    loss_db,
                    noise_figure_db,
                } => {
                    ui.monospace(format!("loss={loss_db:.6} dB\nNF={noise_figure_db:.6} dB"));
                }
                BufferData::Waveform(w) => {
                    ui.label(format!(
                        "{} / {} pts · {} · {:.3} MS/s · simulated={}",
                        w.samples.len(),
                        b.total_samples,
                        w.unit,
                        w.sample_rate_hz / 1e6,
                        w.simulated
                    ));
                    self.buffer_table(ui, &w.samples, None, &w.unit);
                }
                BufferData::Trace(t) => {
                    ui.label(format!(
                        "{} / {} pts · dBm · simulated={}",
                        t.amplitude_dbm.len(),
                        b.total_samples,
                        t.simulated
                    ));
                    self.buffer_table(ui, &t.amplitude_dbm, Some(&t.frequency_hz), "dBm");
                }
                BufferData::Network(t) => {
                    ui.label(format!(
                        "{} · {} / {} pts · dB · simulated={}",
                        t.parameter,
                        t.magnitude_db.len(),
                        b.total_samples,
                        t.simulated
                    ));
                    self.buffer_table(ui, &t.magnitude_db, Some(&t.frequency_hz), "dB");
                }
            }
        }
    }
    fn buffer_table(&self, ui: &mut egui::Ui, values: &[f64], x: Option<&[f64]>, unit: &str) {
        ui.label(pair(
            "Prévisualisation bornée à 4096 échantillons / 512 ports.",
            "Preview limited to 4096 samples / 512 ports.",
        ));
        egui::ScrollArea::vertical()
            .id_salt("buffer-values")
            .max_height(ui.available_height().clamp(120., 350.))
            .show_rows(ui, 20., values.len(), |ui, rows| {
                for i in rows {
                    ui.monospace(if let Some(x) = x {
                        format!("[{i:04}] {:.6} Hz · {:.6} {unit}", x[i], values[i])
                    } else {
                        format!("[{i:04}] {:.8} {unit}", values[i])
                    });
                }
            });
    }
    pub(super) fn help_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui
                .selectable_label(self.help_kind.is_none(), t("Aide globale"))
                .clicked()
            {
                self.help_kind = None;
            }
            ui.add(
                egui::TextEdit::singleline(&mut self.help_search)
                    .hint_text(t("Recherche"))
                    .desired_width(180.),
            );
            egui::ComboBox::from_id_salt("help-kind")
                .selected_text(
                    self.help_kind
                        .map(|k| t(k.label()))
                        .unwrap_or_else(|| t("Aide du bloc")),
                )
                .show_ui(ui, |ui| {
                    for k in editor::search(&self.help_search) {
                        ui.selectable_value(&mut self.help_kind, Some(k), t(k.label()));
                    }
                });
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("help-document")
            .show(ui, |ui| {
                if let Some(k) = self.help_kind {
                    help::block(ui, k);
                } else {
                    help::global(ui);
                }
            });
    }
}
