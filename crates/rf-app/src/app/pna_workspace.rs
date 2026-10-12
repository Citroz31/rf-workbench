use super::*;
use crate::results::{Format, Window};
use rf_core::network::TwoPort;
#[derive(Default)]
pub(super) struct FixtureJob {
    pub node: Option<u64>,
    pub open: bool,
    pub thru: Option<TwoPort>,
    pub raw: Option<TwoPort>,
    left: Option<TwoPort>,
    right: Option<TwoPort>,
    extraction: Option<rf_dsp::fixture::Extraction>,
    thru_path: String,
    raw_path: String,
    left_path: String,
    right_path: String,
    export_path: String,
    reverse_left: bool,
    reverse_right: bool,
    force_z: bool,
    z: f64,
    receiver: Option<std::sync::mpsc::Receiver<Result<FixtureOutput, String>>>,
    message: String,
}
enum FixtureOutput {
    Loaded(Box<FixtureJob>),
    Extracted(Box<rf_dsp::fixture::Extraction>),
    Exported(String),
}
impl FixtureJob {
    fn begin(
        &mut self,
        ctx: egui::Context,
        work: impl FnOnce() -> Result<FixtureOutput, String> + Send + 'static,
    ) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.receiver = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(work());
            ctx.request_repaint();
        });
    }
}
pub(super) fn extra(
    ui: &mut egui::Ui,
    c: &mut rf_core::Config,
    node: u64,
    hardware: bool,
    busy: bool,
    job: &mut Option<Command>,
    fixtures: &mut FixtureJob,
) {
    ui.separator();
    ui.heading("Balayage, chemin RF et calibration");
    ui.checkbox(
        &mut c.instrument.pna.power_sweep,
        "Sweep de puissance à fréquence CW (gain S, application GCA distincte)",
    );
    if c.instrument.pna.power_sweep {
        crate::frequency_edit::frequency(
            ui,
            "Fréquence CW",
            &mut c.frequency_hz,
            &mut c.frequency_unit,
        );
        number(
            ui,
            "Début puissance",
            &mut c.instrument.pna.power_start_dbm,
            0.1,
            " dBm",
        );
        number(
            ui,
            "Fin puissance",
            &mut c.instrument.pna.power_stop_dbm,
            0.1,
            " dBm",
        );
        ui.add(
            egui::DragValue::new(&mut c.instrument.pna.source_port)
                .range(1..=c.instrument.port_count)
                .prefix("Port source "),
        );
    } else {
        number(
            ui,
            "Puissance source du canal",
            &mut c.power_dbm,
            0.1,
            " dBm",
        );
        let mut step = (c.stop_hz - c.start_hz) / (c.points.saturating_sub(1).max(1)) as f64;
        if crate::frequency_edit::frequency(ui, "Pas fréquentiel", &mut step, &mut c.step_unit)
            && let Some(points) = rf_core::frequency::points_for_step(c.start_hz, c.stop_hz, step)
        {
            c.points = points;
        }
        ui.label(format!(
            "Pas effectif : {:.9} {} · {} points (maximum {})",
            (c.stop_hz - c.start_hz)
                / (c.points.saturating_sub(1).max(1) as f64)
                / c.step_unit.factor(),
            c.step_unit.label(),
            c.points,
            rf_core::MAX_POINTS
        ));
        ui.small("Pas effectif = (stop−start)/(points−1) ; les deux extrémités sont conservées.");
        ui.checkbox(
            &mut c.instrument.pna.all_s_parameters,
            "Acquérir toutes les traces Sij (matrice 1–4 ports)",
        );
    }
    ui.small("Configuration source/calibration envoyée seulement si « Appliquer le balayage » est coché. Le déclenchement place le canal en HOLD et acquiert une seule fois. Les limites de puissance sont interrogées avant écriture.");
    number(
        ui,
        "Impédance de référence des S acquis (à vérifier sur l'instrument)",
        &mut c.instrument.pna.reference_ohm,
        1.,
        " Ω",
    );
    ui.label("CalSet existant (nom exact / liste via le bouton)");
    ui.text_edit_singleline(&mut c.instrument.pna.calset);
    ui.checkbox(
        &mut c.instrument.pna.apply_calset,
        "Activer ce CalSet sans remplacer les fréquences du canal",
    );
    if ui
        .add_enabled(
            hardware && !busy,
            egui::Button::new("Lister CalSets de l'appareil"),
        )
        .clicked()
    {
        *job = Some(Command::InstrumentConsole {
            node,
            config: c.clone(),
            command: "CSET:CAT? NAME".into(),
            write: false,
            binary_path: None,
            hardware,
        });
    }
    if ui
        .add_enabled(
            hardware && !busy,
            egui::Button::new("Créer les traces Sij manquantes dans ce canal Standard"),
        )
        .clicked()
    {
        *job = Some(Command::PreparePna {
            node,
            config: c.clone(),
            hardware,
        });
    }
    ui.label("Chemin local des exports de résultats (dossier)");
    ui.text_edit_singleline(&mut c.instrument.pna.result_path);
    ui.horizontal(|ui| {
        if ui.button("Fixtures / 2×Thru temporel…").clicked() {
            fixtures.node = Some(node);
            fixtures.open = true;
            fixtures.reverse_right = true;
            fixtures.z = 50.;
            if fixtures.export_path.is_empty() {
                fixtures.export_path = c.instrument.pna.result_path.clone();
            }
        }
        ui.checkbox(
            &mut c.instrument.pna.fixture.enabled,
            "Déembedding logiciel actif",
        );
    });
    ui.small("L'import et l'extraction sont locaux. Les quatre paramètres complexes sont nécessaires. Les fixtures sont intégrées au fichier .rfbench.");
    ui.separator();
    ui.heading("Limites RF du bloc / DUT");
    limits_ui(ui, &mut c.limits);
}
pub(super) fn limits_ui(ui: &mut egui::Ui, l: &mut rf_core::flow::Limits) {
    let mut band = l.band_hz.is_some();
    if ui.checkbox(&mut band, "Bande RF connue").changed() {
        l.band_hz = band.then_some([10e6, 50e9]);
    }
    if let Some(b) = &mut l.band_hz {
        number(ui, "Min fréquence", &mut b[0], 1e6, " Hz");
        number(ui, "Max fréquence", &mut b[1], 1e6, " Hz");
    }
    for (label, value) in [
        ("Limite entrée (dommage/overload)", &mut l.max_input_dbm),
        ("Limite sortie", &mut l.max_output_dbm),
        ("OP1dB (compression seulement)", &mut l.output_p1db_dbm),
    ] {
        let mut known = value.is_some();
        if ui.checkbox(&mut known, label).changed() {
            *value = known.then_some(0.);
        }
        if let Some(v) = value {
            number(ui, label, v, 0.1, " dBm");
        }
    }
    ui.label("Source / conditions des limites");
    ui.text_edit_singleline(&mut l.source);
}
impl Workbench {
    pub(super) fn fixture_preview(&mut self) {
        let (thru, raw) = rf_dsp::fixture::demo();
        let e = rf_dsp::fixture::split(&thru, Some(50.)).expect("fixture demonstration");
        self.fixture_job = FixtureJob {
            open: true,
            node: self
                .project
                .graph
                .nodes
                .iter()
                .find(|n| n.kind.max_rf_ports().is_some())
                .map(|n| n.id),
            thru: Some(thru),
            raw: Some(raw),
            left: Some(e.left.clone()),
            right: Some(e.right.clone()),
            extraction: Some(e),
            reverse_right: true,
            force_z: true,
            z: 50.,
            ..Default::default()
        };
    }
    pub(super) fn project_tools(&mut self, ui: &mut egui::Ui) {
        if ui.button("Vérifier le banc").clicked() {
            self.flow_report = Some(rf_runtime::preflight(&self.project.graph));
        }
        if ui.button("VNA / résultats").clicked() {
            self.results_open = true;
        }
        ui.menu_button("Panneau inférieur", |ui| {
            for (label, view) in [
                ("Masquer complètement", None),
                ("Paramètres S du PNA", Some(View::Network)),
                ("Spectrum", Some(View::Acquisition)),
            ] {
                if ui.button(label).clicked() {
                    self.layout.panes.retain(|p| p.location != Location::Bottom);
                    if let Some(v) = view {
                        self.layout.set(v, Location::Bottom);
                        self.layout.bottom_active = Some(v);
                    }
                    self.layout_revision += 1;
                    ui.close();
                }
            }
        });
    }
    pub(super) fn flow_window(&mut self, ctx: &egui::Context) {
        let Some(report) = self.flow_report.clone() else {
            return;
        };
        let mut open = true;
        egui::Window::new("Vérification du banc RF").open(&mut open).default_size([740.,450.]).resizable(true).show(ctx,|ui|{
            ui.label(if report.blocked(){"Des incompatibilités bloquent l'exécution."}else{"Aucun blocage détecté ; examiner les limites inconnues et les avertissements."});
            if ui.button("Revérifier après modification").clicked(){self.flow_report=Some(rf_runtime::preflight(&self.project.graph));}
            egui::ScrollArea::vertical().show(ui,|ui|{for f in &report.findings{ui.horizontal_wrapped(|ui|{ui.colored_label(match f.severity{rf_core::flow::Severity::Error=>red(),rf_core::flow::Severity::Warning=>gold(),_=>teal()},format!("{:?}",f.severity));ui.label(&f.message);if let Some(id)=f.node&&ui.small_button("Voir bloc").clicked(){self.canvas.select_only(id);self.view=View::Schematic;}});ui.separator();}});
            ui.small("Estimation linéaire des trajets modélisés. Les P1dB typiques ne sont pas des puissances de dommage. Polarisation DC, connecteurs, MIMO et trajets non modélisés demandent une vérification séparée.");
        });
        if !open {
            self.flow_report = None;
        }
    }
    pub(super) fn add_result_window(
        &mut self,
        node: u64,
        parameter: String,
        format: Format,
        corrected: bool,
    ) {
        if self.layout.result_windows.len() >= 32 {
            return;
        }
        let id = self
            .layout
            .result_windows
            .iter()
            .map(|w| w.id)
            .max()
            .unwrap_or(0)
            + 1;
        let mut window = Window::new(id, node, parameter, format, corrected);
        window.rect = [
            250. + ((id - 1) % 2) as f32 * 560.,
            210. + (((id - 1) / 2) % 2) as f32 * 340.,
            540.,
            290.,
        ];
        window.ensure_traces();
        self.layout.result_windows.push(window);
        self.layout.result_panel.open = true;
        self.layout.result_panel.selected = id;
        self.result_target = Some(id);
    }
    pub(super) fn result_manager(&mut self, ctx: &egui::Context) {
        if !self.results_open {
            return;
        }
        let mut open = true;
        let mut delete = None;
        egui::Window::new("VNA · graphiques et traces").id(egui::Id::new("result-manager"))
            .open(&mut open).default_size([720.,580.]).resizable(true).show(ctx,|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.layout.result_panel.open,"Afficher le panneau VNA");
                ui.checkbox(&mut self.layout.result_panel.floating,"Fenêtres séparées");
            });
            ui.small("Chaque trace conserve sa propre échelle. La trace active définit les graduations visibles ; cliquer sa légende dans le graphique.");
            let mut choices:Vec<(u64,String,bool)>=self.curves.iter().map(|c|(c.node,c.name.clone(),c.corrected)).collect();
            for n in self.project.graph.nodes.iter().filter(|n|n.kind.max_rf_ports().is_some()) {
                for i in 1..=n.config.instrument.port_count {for j in 1..=n.config.instrument.port_count {
                    let item=(n.id,format!("S{i}{j}"),false);if !choices.contains(&item){choices.push(item);}
                }}
            }
            if choices.is_empty(){ui.label("Ajouter un VNA ou acquérir une trace instrument.");return;}
            self.result_selection=self.result_selection.min(choices.len()-1);
            ui.separator();ui.label("Trace à ajouter");
            egui::ComboBox::from_id_salt("result-choice").selected_text(format!("Bloc {} · {}{}",choices[self.result_selection].0,choices[self.result_selection].1,if choices[self.result_selection].2{" corrigé"}else{""})).show_ui(ui,|ui| {
                for(i,(id,p,c))in choices.iter().enumerate(){ui.selectable_value(&mut self.result_selection,i,format!("Bloc {id} · {p} · {}",if *c{"corrigé"}else{"brut"}));}
            });
            egui::ComboBox::from_id_salt("result-target").selected_text(self.result_target.map_or("Nouveau graphique".into(),|id|format!("Graphique {id}"))).show_ui(ui,|ui| {
                ui.selectable_value(&mut self.result_target,None,"Nouveau graphique");
                for w in &self.layout.result_windows{ui.selectable_value(&mut self.result_target,Some(w.id),format!("Graphique {}",w.id));}
            });
            let(n,p,c)=choices[self.result_selection].clone();
            ui.horizontal_wrapped(|ui| {
                for(label,format)in[("Ajouter dB",Format::Magnitude),("Ajouter phase",Format::Phase),("Ajouter Smith",Format::Smith),("Ajouter valeur Y",Format::Linear)] {
                    if ui.button(label).clicked() {
                        if let Some(w)=self.layout.result_windows.iter_mut().find(|w|Some(w.id)==self.result_target){w.add_trace(n,p.clone(),format,c);}
                        else {self.add_result_window(n,p.clone(),format,c);}
                        self.layout.result_panel.open=true;
                    }
                }
                if ui.button("Créer un autre graphique").clicked(){self.result_target=None;}
            });
            if let Some(w)=self.layout.result_windows.iter_mut().find(|w|Some(w.id)==self.result_target) {
                w.ensure_traces();ui.separator();ui.heading(format!("Graphique {} · {} traces",w.id,w.traces.len()));
                let mut remove=None;
                egui::ScrollArea::vertical().max_height(200.).show(ui,|ui| {
                    for(i,t)in w.traces.iter_mut().enumerate(){ui.horizontal_wrapped(|ui| {
                        ui.checkbox(&mut t.visible,"");
                        if ui.selectable_label(w.active_trace==i,egui::RichText::new(format!("{} · bloc {}",t.parameter,t.node)).color(crate::vna_panel::color(t.color))).clicked(){w.active_trace=i;}
                        egui::ComboBox::from_id_salt(("format",w.id,i)).selected_text(format!("{:?}",t.format)).show_ui(ui,|ui|{for f in [Format::Magnitude,Format::Phase,Format::Smith,Format::Linear]{ui.selectable_value(&mut t.format,f,format!("{f:?}"));}});
                        if ui.small_button("Retirer").clicked(){remove=Some(i);}
                    });}
                });
                if let Some(i)=remove {w.traces.remove(i);if w.traces.is_empty(){delete=Some(w.id);}else{w.active_trace=w.active_trace.min(w.traces.len()-1);}}
                if !w.traces.is_empty(){crate::vna_panel::scale_controls(ui,w,&self.curves);}
                if ui.button("Fermer ce graphique").clicked(){delete=Some(w.id);}
            }
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                if ui.button("Panel 4 graphes PNA").clicked(){
                    self.layout.result_windows.clear();
                    for(param,f)in[("S11",Format::Magnitude),("S21",Format::Magnitude),("S11",Format::Smith),("S21",Format::Phase)]{self.add_result_window(n,param.into(),f,c);}
                    self.layout.result_panel.columns=2;self.layout.result_panel.rows=2;self.layout.result_panel.floating=false;
                }
                if ui.button("S11 + S22 ensemble").clicked(){
                    self.add_result_window(n,"S11".into(),Format::Magnitude,c);
                    self.layout.result_windows.last_mut().unwrap().add_trace(n,"S22".into(),Format::Magnitude,c);
                    self.layout.result_panel.columns=1;self.layout.result_panel.rows=1;
                }
                if ui.button("Fermer tous les graphiques").clicked(){self.layout.result_windows.clear();}
            });
            ui.small("Pour acquérir S11 et S22 ensemble, activer la matrice Sij dans le bloc. NF/IMD/R1/B1 utilisent les traces FDATA préparées sur l'appareil. Smith requiert une réflexion complexe ; ne pas mélanger Smith et tracés rectangulaires dans un même graphique.");
            let mut folder=self.project.graph.node(n).map(|n|n.config.instrument.pna.result_path.clone()).unwrap_or_default();
            ui.label("Dossier local des exports de ce bloc");
            if ui.text_edit_singleline(&mut folder).changed()&&let Some(node)=self.project.graph.nodes.iter_mut().find(|node|node.id==n){node.config.instrument.pna.result_path=folder.clone();}
            if ui.add_enabled(!self.worker.is_busy()&&!folder.trim().is_empty()&&!self.curves.is_empty(),egui::Button::new("Exporter les résultats JSON")).clicked(){self.submit(Command::ExportResults{curves:self.curves.iter().filter(|c|c.node==n).cloned().collect(),folder,project:self.project.name.clone()});}
        });
        if let Some(id) = delete {
            self.layout.result_windows.retain(|w| w.id != id);
        }
        self.results_open = open;
    }
    pub(super) fn pna_result_windows(&mut self, ctx: &egui::Context) {
        let out = crate::vna_panel::show(
            ctx,
            &mut self.layout.result_panel,
            &mut self.layout.result_windows,
            &self.curves,
            self.layout_revision,
        );
        if let Some(id) = out.configure {
            self.layout.result_panel.selected = id;
            self.result_target = Some(id);
            self.results_open = true;
        }
        self.layout
            .result_windows
            .retain(|w| !out.closed.contains(&w.id));
    }
    pub(super) fn poll_fixture(&mut self) {
        let Some(rx) = &self.fixture_job.receiver else {
            return;
        };
        if let Ok(result) = rx.try_recv() {
            self.fixture_job.receiver = None;
            match result {
                Ok(FixtureOutput::Loaded(j)) => {
                    self.fixture_job.thru = j.thru;
                    self.fixture_job.raw = j.raw;
                    self.fixture_job.left = j.left;
                    self.fixture_job.right = j.right;
                    self.fixture_job.extraction = None;
                    self.fixture_job.message =
                        "Fichiers s2p chargés, grilles et impédances à vérifier avant correction."
                            .into();
                }
                Ok(FixtureOutput::Extracted(e)) => {
                    self.fixture_job.left = Some(e.left.clone());
                    self.fixture_job.right = Some(e.right.clone());
                    self.fixture_job.reverse_left = false;
                    self.fixture_job.reverse_right = true;
                    self.fixture_job.message = format!(
                        "Fixtures extraites · résidu {:.6} dB / {:.6}°",
                        e.residual_db, e.residual_deg
                    );
                    self.fixture_job.extraction = Some(*e);
                }
                Ok(FixtureOutput::Exported(path)) => {
                    self.fixture_job.message = format!("Export local : {path}")
                }
                Err(e) => self.fixture_job.message = e,
            }
        }
    }
    pub(super) fn fixture_window(&mut self, ctx: &egui::Context) {
        if !self.fixture_job.open {
            return;
        }
        let mut j = std::mem::take(&mut self.fixture_job);
        let mut open = true;
        let busy = j.receiver.is_some();
        let mut command = None;
        let mut apply = false;
        egui::Window::new("Fixtures · 2×Thru temporel / fichiers s2p").id(egui::Id::new("fixtures")).open(&mut open).default_pos([260.,100.]).default_size([860.,650.]).max_height((ctx.content_rect().height()-120.).max(350.)).resizable(true).show(ctx,|ui|{egui::ScrollArea::vertical().show(ui,|ui|{
            ui.label("Connecteur 1 -> fixture entrée -> DUT -> fixture sortie inversée -> connecteur 2");
            ui.label("Calibrer le PNA aux connecteurs des fixtures avant la mesure. Port 1/2, canal Standard, traces S11/S21/S12/S22 existantes. Les sweeps utilisent les réglages appliqués au bloc.");
            if let Some(n)=j.node.and_then(|id|self.project.graph.node(id)){ui.horizontal_wrapped(|ui|{for(label,thru)in[("Mesurer 2×Thru connecté",true),("Mesurer fixture–DUT–fixture",false)]{if ui.add_enabled(self.hardware&&!self.worker.is_busy()&&!busy,egui::Button::new(label)).clicked(){command=Some(Command::PnaFixture{node:n.id,config:n.config.clone(),hardware:self.hardware,thru});}}});}
            for(label,path)in[("Fichier 2×Thru .s2p",&mut j.thru_path),("Fichier fixture–DUT–fixture .s2p",&mut j.raw_path),("Fixture entrée .s2p (optionnel)",&mut j.left_path),("Fixture sortie .s2p (optionnel)",&mut j.right_path)]{ui.label(label);ui.add(egui::TextEdit::singleline(path).desired_width(f32::INFINITY));}
            ui.horizontal_wrapped(|ui|{if ui.add_enabled(!busy,egui::Button::new("Charger les fichiers indiqués")).clicked(){let paths=[j.thru_path.clone(),j.raw_path.clone(),j.left_path.clone(),j.right_path.clone()];j.begin(ctx.clone(),move||{let mut loaded=FixtureJob::default();let data=paths.iter().map(|p|if p.trim().is_empty(){Ok(None)}else{read_limited(p,2_000_000).and_then(|text|rf_dsp::fixture::parse(&text)).map(Some)}).collect::<Result<Vec<_>,String>>()?;if data.iter().all(Option::is_none){return Err("Indiquer un fichier local".into());}let mut it=data.into_iter();loaded.thru=it.next().unwrap();loaded.raw=it.next().unwrap();loaded.left=it.next().unwrap();loaded.right=it.next().unwrap();Ok(FixtureOutput::Loaded(Box::new(loaded)))});}
            if ui.add_enabled(!busy,egui::Button::new("Démonstration locale 2×Thru + DUT 20 dB")).clicked(){let(a,b)=rf_dsp::fixture::demo();j.thru=Some(a);j.raw=Some(b);j.extraction=None;j.left=None;j.right=None;j.z=50.;j.force_z=true;j.message="Données simulées 0.1–25.6 GHz ; le gain 20 dB est un exemple mathématique.".into();}});
            ui.checkbox(&mut j.force_z,"Fixer l'impédance du plan de coupe (sinon estimation TDR)");if j.force_z{number(ui,"Z au plan de coupe",&mut j.z,1.," Ω");}
            if ui.add_enabled(!busy&&j.thru.is_some(),egui::Button::new("Extraire les demi-fixtures (IFFT -> gate -> FFT)")).clicked(){let thru=j.thru.clone().unwrap();let z=j.force_z.then_some(j.z);j.begin(ctx.clone(),move||rf_dsp::fixture::split(&thru,z).map(|e|FixtureOutput::Extracted(Box::new(e))));}
            ui.horizontal(|ui|{ui.checkbox(&mut j.reverse_left,"Reverse entrée : ports 1 ↔ 2");ui.checkbox(&mut j.reverse_right,"Reverse sortie : ports 1 ↔ 2");});
            if let Some(e)=&j.extraction{ui.label(format!("Coupe : {:.3} ps · Z={:.3} Ω · résidu max {:.6} dB / {:.6}°",e.split_time_s*1e12,e.split_impedance_ohm,e.residual_db,e.residual_deg));for w in &e.warnings{ui.small(w);}plot::series(ui,&e.time_s,&e.transmission_impulse,160.,"Temps (ns)","Impulsion S21",1e-9);plot::series(ui,&e.time_s,&e.impedance_ohm,160.,"Temps (ns)","TDR (Ω)",1e-9);}
            if ui.add_enabled(!busy&&j.raw.is_some()&&j.left.is_some()&&j.right.is_some(),egui::Button::new("Corriger le fichier DUT et ouvrir les résultats")).clicked(){let result=rf_dsp::fixture::deembed(j.raw.as_ref().unwrap(),j.left.as_ref().unwrap(),j.right.as_ref().unwrap(),j.reverse_left,j.reverse_right);match result{Ok(d)=>{let id=j.node.unwrap_or(0);self.curves.retain(|c|c.node!=id);for p in ["S11","S21","S12","S22"]{for(data,corrected)in[(j.raw.as_ref().unwrap(),false),(&d,true)]{if let Ok(t)=data.trace(p){let mut c=rf_core::network::Curve::from_trace(id,&t);c.corrected=corrected;self.curves.push(c);}}}self.results_open=true;j.message="Résultats bruts et corrigés disponibles dans Résultats.".into();},Err(e)=>j.message=e}}
            apply=ui.add_enabled(!busy&&j.node.is_some()&&j.left.is_some()&&j.right.is_some(),egui::Button::new("Intégrer ces fixtures au bloc PNA et au projet")).clicked();
            ui.label("Dossier d'export local neuf (fixtures, brut, corrigé, rapport)");ui.text_edit_singleline(&mut j.export_path);
            if ui.add_enabled(!busy&&j.left.is_some()&&j.right.is_some()&&!j.export_path.trim().is_empty(),egui::Button::new("Exporter les s2p et le rapport")).clicked(){let left=j.left.clone().unwrap();let right=j.right.clone().unwrap();let raw=j.raw.clone();let(a,b)=(j.reverse_left,j.reverse_right);let path=j.export_path.clone();let extraction=j.extraction.clone();j.begin(ctx.clone(),move||{
                let mut files=vec![("fixture-input.s2p",rf_dsp::fixture::touchstone(&left)?),("fixture-output.s2p",rf_dsp::fixture::touchstone(&right)?)];if let Some(raw)=raw{let corrected=rf_dsp::fixture::deembed(&raw,&left,&right,a,b)?;files.push(("raw.s2p",rf_dsp::fixture::touchstone(&raw)?));files.push(("dut-corrected.s2p",rf_dsp::fixture::touchstone(&corrected)?));}
                let report=serde_json::json!({"method":"NZC-style native time gating; not IEEE certification","reverse_input":a,"reverse_output":b,"split_time_s":extraction.as_ref().map(|e|e.split_time_s),"split_z_ohm":extraction.as_ref().map(|e|e.split_impedance_ohm),"residual_db":extraction.as_ref().map(|e|e.residual_db),"residual_deg":extraction.as_ref().map(|e|e.residual_deg),"warnings":extraction.as_ref().map(|e|e.warnings.clone())});files.push(("report.json",serde_json::to_string_pretty(&report).map_err(|e|e.to_string())?));std::fs::create_dir(&path).map_err(|e|format!("Dossier neuf requis : {e}"))?;for(name,text)in files{std::fs::write(std::path::Path::new(&path).join(name),text).map_err(|e|format!("Export partiel dans {path}: {e}"))?;}Ok(FixtureOutput::Exported(path))});}
            if busy{ui.label("Calcul / lecture en arrière-plan…");}ui.label(&j.message);
        });});
        if apply && let Some(id) = j.node {
            let result = (|| -> Result<rf_core::instrument::Fixture, String> {
                Ok(rf_core::instrument::Fixture {
                    enabled: true,
                    input_s2p: rf_dsp::fixture::touchstone(j.left.as_ref().unwrap())?,
                    output_s2p: rf_dsp::fixture::touchstone(j.right.as_ref().unwrap())?,
                    reverse_input: j.reverse_left,
                    reverse_output: j.reverse_right,
                    provenance: "Fixtures importées / NZC temporel local ; contrôler le rapport"
                        .into(),
                })
            })();
            match result {
                Ok(fixture) => {
                    self.history.record(self.project.graph.clone());
                    if let Some(n) = self.project.graph.nodes.iter_mut().find(|n| n.id == id) {
                        if n.config.instrument.port_count != 2 {
                            j.message =
                                "Configurer le bloc PNA à 2 ports avant d'appliquer ces fixtures"
                                    .into();
                        } else {
                            n.config.instrument.pna.fixture = fixture.clone();
                            n.config.instrument.pna.all_s_parameters = true;
                            if let Some(d) = &mut self.instrument_dialog
                                && d.id == id
                            {
                                d.config.instrument.pna.fixture = fixture;
                                d.config.instrument.pna.all_s_parameters = true;
                            }
                            j.message="Fixtures intégrées ; enregistrer le projet. Les quatre Sij seront corrigés après acquisition.".into();
                        }
                    }
                }
                Err(e) => j.message = e,
            }
        }
        j.open = open;
        self.fixture_job = j;
        if let Some(cmd) = command {
            self.submit(cmd);
        }
    }
}
