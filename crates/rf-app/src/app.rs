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
mod dsp;
mod instrument;
#[cfg(test)]
mod instrument_tests;
mod pna_workspace;
mod professional;
mod projects;
mod workspace;
use crate::{
    analysis::Waterfall,
    i18n::{pair, t},
    studio::{Layout, Location, Studio, Workspace},
};
use rf_runtime::debug::{Buffer, Snapshot};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) enum View {
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
    Waterfall,
    Constellation,
    Smith,
    Eye,
    Timing,
    Debug,
    Studio,
    Help,
    Dsp,
}
impl View {
    pub const DOCKS: [Self; 11] = [
        Self::Schematic,
        Self::Acquisition,
        Self::Network,
        Self::Measurements,
        Self::Waterfall,
        Self::Constellation,
        Self::Smith,
        Self::Eye,
        Self::Timing,
        Self::Debug,
        Self::Dsp,
    ];
    pub fn dockable(self) -> bool {
        Self::DOCKS.contains(&self)
    }
    pub fn label(self) -> String {
        t(match self {
            Self::Schematic => "Schéma",
            Self::Acquisition => "Spectre",
            Self::Network => "Paramètres S",
            Self::Measurements => "Mesures",
            Self::Waterfall => "Waterfall",
            Self::Constellation => "Constellation",
            Self::Smith => "Smith",
            Self::Eye => "Diagramme de l'œil",
            Self::Timing => "Chronogramme",
            Self::Debug => "Débogage",
            Self::Library => "Blocs",
            Self::Tests => "Tests",
            Self::Python => "Python",
            Self::Instruments => "SCPI",
            Self::DutCatalog => "DUT",
            Self::Project => "Projet",
            Self::Settings => "Raccourcis",
            Self::Studio => "Espaces de travail",
            Self::Help => "Aide",
            Self::Dsp => "RF / DSP",
        })
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum TraceDisplay {
    Preview,
    Acquired,
    Unavailable,
}
pub struct Workbench {
    registry: crate::project_registry::Registry,
    registry_path: String,
    active_project: bool,
    last_saved: String,
    dialog_path: String,
    pending_project: Option<(crate::bench_file::BenchFile, String, bool)>,
    curves: Vec<rf_core::network::Curve>,
    flow_report: Option<rf_core::flow::Report>,
    results_open: bool,
    result_selection: usize,
    fixture_job: pna_workspace::FixtureJob,
    data_directory: crate::paths::DataDirectory,
    welcome: bool,
    project_dialog: Option<bool>,
    browse_directory: String,
    file_browser: crate::file_browser::Browser,
    instrument_dialog: Option<instrument::Dialog>,
    context: egui::Context,
    studio: Studio,
    layout: Layout,
    studio_path: String,
    layout_name: String,
    workspace_name: String,
    setup_rename: Option<professional::RenameSetup>,
    buffers: Vec<Buffer>,
    debug_snapshot: Option<Snapshot>,
    buffer_key: Option<(u64, usize)>,
    probes_only: bool,
    waterfall: Waterfall,
    iq_keys: [Option<(u64, usize)>; 2],
    wave_key: Option<(u64, usize)>,
    samples_per_symbol: usize,
    sample_offset: usize,
    trajectory: bool,
    threshold: f64,
    z0: f64,
    help_kind: Option<Kind>,
    help_search: String,
    clipboard: Option<rf_core::Graph>,
    appearance: Option<(bool, bool, f32, crate::studio::Language)>,
    layout_revision: u64,
    routing_job: Option<crate::editor::RoutingJob>,
    capture_cycles: Option<u64>,
    capture_path: Option<String>,
    capture_frames: u32,
    capture_requested: bool,
    project: Project,
    canvas: Canvas,
    history: History,
    worker: Worker,
    view: View,
    trace: Trace,
    trace_display: TraceDisplay,
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
    resources: Vec<String>,
    dsp_draft: dsp::Draft,
    dsp_iq: Option<u64>,
    dsp_spectrum: Option<u64>,
    dsp_history: dsp::History,
    dsp_tab: u8,
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
        let data_directory = crate::paths::DataDirectory::initialize();
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
        let project_path = data_directory.file("bench.rfbench");
        let csv_path = data_directory.file("measurement.csv");
        let preferences_path = data_directory.file("preferences.rfw.json");
        let preferences = read_limited(&preferences_path, 64_000)
            .ok()
            .filter(|s| s.len() <= 64_000)
            .and_then(|s| serde_json::from_str::<Preferences>(&s).ok())
            .map(Preferences::upgrade)
            .filter(|p| p.validate().is_ok())
            .unwrap_or_default();
        let catalog_path = data_directory.file("dut-catalog.json");
        let mut catalog = read_limited(&catalog_path, 4_000_000)
            .ok()
            .and_then(|s| Catalog::from_json(&s).ok())
            .unwrap_or_default();
        catalog.include_bundled();
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
        if args.iter().any(|a| a == "--demo-dsp") {
            project.graph = rf_runtime::dsp_demo();
            project.name = "RF / DSP · QAM16 & canal".into();
        }
        if args.iter().any(|a| a == "--demo-pa") {
            project.graph = rf_core::Graph::pa_demo();
            project.name = "PA 36–38 GHz · gain 20 dB (simulation)".into();
        }
        let view = if args.iter().any(|a| a == "--gallery") {
            View::Library
        } else if args.iter().any(|a| a == "--settings") {
            View::Settings
        } else {
            View::Schematic
        };
        let studio_path = data_directory.file("studio.rfw.json");
        let studio = read_limited(&studio_path, 32_000_000)
            .ok()
            .and_then(|s| Studio::from_json(&s).ok())
            .unwrap_or_default();
        let registry_path = data_directory.file("recent-projects.json");
        let registry = crate::project_registry::Registry::read(&registry_path);
        let mut bench = Self {
            registry,
            registry_path,
            active_project: false,
            last_saved: String::new(),
            dialog_path: project_path.clone(),
            pending_project: None,
            curves: Vec::new(),
            flow_report: None,
            results_open: false,
            result_selection: 0,
            fixture_job: Default::default(),
            welcome: !args.iter().any(|a| {
                a.starts_with("--demo-")
                    || [
                        "--gallery",
                        "--dsp",
                        "--dashboard",
                        "--settings",
                        "--studio",
                        "--help-ui",
                        "--debug-ui",
                        "--instrument-ui",
                        "--project-ui",
                        "--open",
                    ]
                    .contains(&a.as_str())
            }),
            project_dialog: None,
            file_browser: Default::default(),
            browse_directory: data_directory.path().to_string_lossy().into_owned(),
            data_directory,
            instrument_dialog: None,
            setup_rename: None,
            context: cc.egui_ctx.clone(),
            layout: Layout::default(),
            studio,
            studio_path,
            layout_name: "Banc RF".into(),
            workspace_name: "Nouveau banc".into(),
            buffers: Vec::new(),
            debug_snapshot: None,
            buffer_key: None,
            probes_only: false,
            waterfall: Waterfall::default(),
            iq_keys: [None, None],
            wave_key: None,
            samples_per_symbol: 16,
            sample_offset: 0,
            trajectory: true,
            threshold: 0.,
            z0: 50.,
            help_kind: None,
            help_search: String::new(),
            clipboard: None,
            appearance: None,
            layout_revision: 0,
            routing_job: None,
            capture_cycles: None,
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
            resources: vec![],
            dsp_draft: dsp::Draft::default(),
            dsp_iq: None,
            dsp_spectrum: None,
            dsp_history: dsp::History::default(),
            dsp_tab: if args.iter().any(|a| a == "--dsp-spectrum") {
                1
            } else {
                0
            },
            component_draft: Component {
                id: String::new(),
                manufacturer: "Local".into(),
                part_number: String::new(),
                category: "Composant RF".into(),
                description: String::new(),
                insertion_loss_db: Some(3.),
                noise_figure_db: Some(2.5),
                sources: Vec::new(),
                rf: None,
                attributes: Default::default(),
            },
            canvas: Canvas::configured(preferences.snap, preferences.orthogonal),
            history: History::default(),
            worker,
            view,
            trace,
            trace_display: TraceDisplay::Preview,
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
        if let Some(warning) = bench.data_directory.warning().map(str::to_owned) {
            bench.log(warning, true);
        }
        // Legacy Studio files are not reopened as project tabs. Keep a recoverable
        // copy before preferences are rewritten by the new project lifecycle.
        if !bench.studio.workspaces.is_empty() {
            let backup = bench
                .data_directory
                .file("studio-before-projects-v070.json");
            if !std::path::Path::new(&backup).exists()
                && let Err(error) = serde_json::to_string_pretty(&bench.studio)
                    .map_err(|e| e.to_string())
                    .and_then(|text| atomic_save(&backup, &text))
            {
                // Preserve the original collection even when its backup fails.
                bench.studio_path = bench
                    .data_directory
                    .file("studio-project-preferences-v070.json");
                bench.log(
                    format!(
                        "Ancien Studio conservé : sauvegarde de migration impossible ({error})."
                    ),
                    true,
                );
            }
        }
        bench.studio.workspaces = vec![Workspace::new(
            bench.project.name.clone(),
            bench.project.clone(),
        )];
        bench.studio.active = 0;
        bench.active_project = !bench.welcome;
        if let Some(i) = args.iter().position(|a| a == "--open")
            && let Some(path) = args.get(i + 1)
        {
            bench.dialog_path = path.clone();
            match read_limited(&bench.dialog_path, 4_000_000)
                .and_then(|s| crate::bench_file::BenchFile::parse(&s))
            {
                Ok(file) => bench.install_project(file, bench.dialog_path.clone(), false),
                Err(e) => {
                    bench.welcome = true;
                    bench.log(e, true);
                }
            }
        }
        if !args.iter().any(|a| a == "--open")
            && let Some(path) = args
                .iter()
                .skip(1)
                .find(|a| a.to_lowercase().ends_with(".rfbench"))
        {
            bench.dialog_path = path.clone();
            match read_limited(&bench.dialog_path, 4_000_000)
                .and_then(|s| crate::bench_file::BenchFile::parse(&s))
            {
                Ok(file) => bench.install_project(file, bench.dialog_path.clone(), false),
                Err(e) => {
                    bench.welcome = true;
                    bench.log(e, true);
                }
            }
        }
        if args.iter().any(|a| a == "--welcome") {
            bench.welcome = true;
        }
        if args.iter().any(|a| a == "--project-ui") {
            bench.project_dialog = Some(false);
        }
        if args.iter().any(|a| a == "--instrument-ui")
            && let Some(id) = bench
                .project
                .graph
                .nodes
                .iter()
                .find(|n| matches!(n.kind, Kind::Pna | Kind::PnaX))
                .map(|n| n.id)
        {
            bench.open_instrument(id);
            if args.iter().any(|a| a == "--instrument-functions")
                && let Some(d) = &mut bench.instrument_dialog
            {
                d.tab = 1;
                d.config.instrument.configure_sweep = true;
            }
        }
        if args.iter().any(|a| a == "--studio") {
            bench.view = View::Studio;
        }
        if args.iter().any(|a| a == "--catalog-ui") {
            bench.view = View::DutCatalog;
            bench.welcome = false;
        }
        if args.iter().any(|a| a == "--dut-ui" || a == "--dc-ui") {
            let dc = args.iter().any(|a| a == "--dc-ui");
            if let Some(id) = bench
                .project
                .graph
                .nodes
                .iter()
                .find(|n| {
                    if dc {
                        n.kind.is_dc_supply()
                    } else {
                        n.kind == Kind::Dut
                    }
                })
                .map(|n| n.id)
            {
                bench.open_instrument(id);
                if let Some(d) = &mut bench.instrument_dialog {
                    d.tab = 1;
                }
            }
        }
        if args.iter().any(|a| a == "--help-ui") {
            bench.view = View::Help;
        }
        if args.iter().any(|a| a == "--light") {
            bench.studio.light = true;
        }
        if args.iter().any(|a| a == "--english") {
            bench.studio.language = crate::studio::Language::English;
        }
        for (flag, view) in [
            ("--pa-ui", View::Network),
            ("--dsp", View::Dsp),
            ("--waterfall", View::Waterfall),
            ("--constellation", View::Constellation),
            ("--smith", View::Smith),
            ("--eye", View::Eye),
            ("--timing", View::Timing),
            ("--debug-ui", View::Debug),
        ] {
            if args.iter().any(|a| a == flag) {
                bench.view = view;
                bench.layout.panes.clear();
            }
        }
        if args.iter().any(|a| a == "--smith") {
            for n in &mut bench.project.graph.nodes {
                if matches!(n.kind, Kind::Pna | Kind::PnaX) {
                    n.config.s_parameter = "S11".into();
                }
            }
        }
        if args.iter().any(|a| a == "--dashboard") {
            bench.studio.inspector = false;
            bench.layout.panes.clear();
            bench.layout.set(View::Debug, Location::Right);
            bench.layout.set(View::Acquisition, Location::Bottom);
        }
        if args.iter().any(|a| a == "--route-demo") {
            bench.start_routing(args.iter().any(|a| a == "--organize-demo"), false, true);
        }
        if let Some(index) = args.iter().position(|a| a == "--capture-cycles") {
            bench.capture_cycles = args
                .get(index + 1)
                .and_then(|n| n.parse().ok())
                .filter(|n| *n > 0 && *n <= 128);
            bench.continuous = bench.capture_cycles.is_some();
        }
        if args.iter().any(|a| a == "--start-debug") {
            bench.debug_start();
        }
        if args.iter().any(|a| a == "--results-ui")
            && let Some(n) = bench
                .project
                .graph
                .nodes
                .iter_mut()
                .find(|n| n.kind.max_rf_ports().is_some())
        {
            n.config.instrument.pna.all_s_parameters = true;
            let id = n.id;
            bench.add_result_window(id, "S11".into(), crate::results::Format::Magnitude, false);
            bench.add_result_window(id, "S11".into(), crate::results::Format::Smith, false);
            bench.add_result_window(id, "S21".into(), crate::results::Format::Phase, false);
        }
        if args.iter().any(|a| a == "--fixture-ui") {
            bench.fixture_preview();
            bench.welcome = false;
        }
        if args.iter().any(|a| a == "--flow-ui") {
            bench.flow_report = Some(rf_runtime::preflight(&bench.project.graph));
            bench.welcome = false;
        }
        if args.iter().any(|a| a == "--run-demo") {
            bench.run();
            if args.iter().any(|a| a == "--results")
                && !args.iter().any(|a| {
                    matches!(
                        a.as_str(),
                        "--waterfall"
                            | "--constellation"
                            | "--smith"
                            | "--eye"
                            | "--timing"
                            | "--debug-ui"
                    )
                })
            {
                bench.view = if args.iter().any(|a| a == "--demo-dsp") {
                    View::Dsp
                } else if args.iter().any(|a| a == "--demo-pna") {
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
        let report = rf_runtime::preflight(&self.project.graph);
        let blocked = report.blocked();
        if blocked || self.flow_report.is_some() {
            self.flow_report = Some(report);
        }
        if blocked {
            self.log(
                "Exécution bloquée : consulter Vérifier le banc".into(),
                true,
            );
            return;
        }
        self.submit(Command::Run {
            graph: self.project.graph.clone(),
            continuous: self.continuous,
            python_path: self.python_path.clone(),
            hardware: self.hardware,
        });
    }
    fn result(&mut self, r: RunResult) {
        self.preview = false;
        self.trace_display = if r.trace.is_some() {
            TraceDisplay::Acquired
        } else {
            TraceDisplay::Unavailable
        };
        if r.overflows > 0 {
            self.log(
                format!("Streaming : {} trames perdues (queue pleine)", r.overflows),
                true,
            );
        }
        self.dsp_history.push(&r.buffers);
        self.buffers = r.buffers;
        self.debug_snapshot = None;
        self.canvas.active_node = None;
        if let Some(t) = r.trace {
            self.waterfall.push(&t);
            self.trace = t;
            self.preview = false;
        }
        self.curves = r.curves;
        self.network = r.network;
        self.waveform = r.waveform;
        self.measurements = r.measurements;
        self.tests = r.tests;
        self.canvas.completed = r.completed;
        self.elapsed = r.elapsed_ms;
        self.sequence = r.sequence + 1;
        if self.capture_cycles.is_some_and(|n| self.sequence >= n) {
            self.worker.stop();
        }
    }
    fn poll(&mut self) {
        self.poll_routing();
        if let Some(snapshot) = self.worker.take_debug() {
            self.buffers = snapshot.buffers.clone();
            self.canvas.completed = snapshot.completed.clone();
            self.canvas.active_node = Some(snapshot.node);
            self.debug_snapshot = Some(snapshot);
        }
        if let Some(r) = self.worker.take_latest() {
            self.result(r);
        }
        let events: Vec<_> = self.worker.events.try_iter().collect();
        for event in events {
            match event {
                Event::ProjectReset(errors) => {
                    if errors.is_empty() {
                        if let Some((file, path, is_new)) = self.pending_project.take() {
                            self.install_project(file, path, is_new);
                        }
                    } else {
                        self.pending_project = None;
                        for e in errors {
                            self.log(e, true);
                        }
                        self.log(
                            "Changement de projet annulé : arrêt des sorties non confirmé".into(),
                            true,
                        );
                    }
                }
                Event::PnaCurves(curves) => {
                    self.curves.extend(curves);
                    self.results_open = true;
                }
                Event::PnaFixtureData { node, data, thru } => {
                    if self.fixture_job.node == Some(node) {
                        if thru {
                            self.fixture_job.thru = Some(data);
                        } else {
                            self.fixture_job.raw = Some(data);
                        }
                        self.fixture_job.open = true;
                        self.log(
                            "Matrice 2 ports acquise en binaire ; aucune extraction automatique"
                                .into(),
                            false,
                        );
                    }
                }
                Event::PnaApplicationData { node, data } => {
                    let curve = rf_core::network::Curve {
                        node,
                        name: format!("{} · {}", data.measurement, data.parameter),
                        x: data.x.clone(),
                        x_unit: "instrument".into(),
                        y: data.y.clone(),
                        y_unit: format!("format {}", data.display_format),
                        phase_deg: None,
                        simulated: false,
                        corrected: false,
                    };
                    self.curves
                        .retain(|c| c.node != node || c.name != curve.name);
                    self.curves.push(curve);
                    if let Some(d) = &mut self.instrument_dialog
                        && d.id == node
                    {
                        d.application = Some(data);
                        d.message="FDATA acquise ; unités de l’affichage instrument, axe dans son domaine actuel.".into();
                    }
                }
                Event::Devices { node, devices } => {
                    if let Some(d) = &mut self.instrument_dialog
                        && d.id == node
                    {
                        d.accept_devices(devices);
                    }
                }
                Event::PnaCapabilities { node, capabilities } => {
                    if let Some(d) = &mut self.instrument_dialog
                        && d.id == node
                    {
                        d.capabilities = Some(capabilities);
                        d.message = "Identification / capacités mises à jour.".into();
                    }
                }
                Event::InstrumentResponse { node, text } => {
                    self.log(text.clone(), false);
                    if let Some(d) = &mut self.instrument_dialog
                        && d.id == node
                    {
                        d.output = text;
                    }
                }
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
                Event::Resources(v) => {
                    self.log(format!("{} ressources VISA découvertes", v.len()), false);
                    self.resources = v;
                }
                Event::Message(s) => self.log(s, false),
                Event::Error(e) => self.log(e, true),
                Event::Idle => {
                    if let Some(snapshot) = &mut self.debug_snapshot {
                        snapshot.paused = false;
                    }
                    self.canvas.active_node = None;
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
        let path = crate::bench_file::project_path(if self.dialog_path.is_empty() {
            &self.project_path
        } else {
            &self.dialog_path
        });
        match crate::bench_file::BenchFile::new(self.project.clone(), self.layout.clone())
            .json()
            .and_then(|s| {
                atomic_save(&path, &s)?;
                self.last_saved = s;
                Ok(())
            }) {
            Ok(()) => {
                self.project_path = path.clone();
                self.dialog_path = path;
                self.remember_project();
                self.log(format!("Projet enregistré : {}", self.project_path), false);
            }
            Err(e) => self.log(e, true),
        }
    }
    fn load(&mut self) {
        if self.worker.is_busy() {
            self.log("Arrêter la tâche avant de changer de projet".into(), true);
            return;
        }
        let path = self.dialog_path.clone();
        match read_limited(&path, 4_000_000).and_then(|s| crate::bench_file::BenchFile::parse(&s)) {
            Ok(file) => self.request_project(file, path, false),
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
        if kind == Kind::PnaX
            && let Some(n) = self.project.graph.nodes.iter_mut().find(|n| n.id == id)
        {
            n.config.instrument.expected_idn = "N5245B".into();
        }
        self.canvas.select_only(id);
        self.studio.used(kind);
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
                    self.dialog_path = self.project_path.clone();
                    self.project_dialog = Some(true);
                }
            }
            Action::Open => {
                if !self.worker.is_busy() {
                    self.project_dialog = Some(false);
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
            Action::Delete => self.delete_selection(),
            Action::Duplicate => {
                let clip = crate::editor::selection(&self.project.graph, &self.canvas.selected_ids);
                self.paste_graph(&clip);
            }
            Action::Copy => self.copy_selection(),
            Action::Paste => {
                if let Some(clip) = self.clipboard.clone() {
                    self.paste_graph(&clip);
                }
            }
            Action::SelectAll => {
                self.canvas.selected_ids = self.project.graph.nodes.iter().map(|n| n.id).collect();
                self.canvas.selected = self.canvas.selected_ids.iter().next().copied();
            }
            Action::AutoLayout => self.organize(),
            Action::Route => self.route_all(),
            Action::Annotate => self.add_annotation(),
            Action::Help => {
                self.help_kind = None;
                self.view = View::Help;
            }
            Action::BlockHelp => {
                self.help_kind = self
                    .canvas
                    .selected
                    .and_then(|id| self.project.graph.node(id))
                    .map(|n| n.kind);
                self.view = View::Help;
            }
            Action::DebugStart => self.debug_start(),
            Action::DebugStep => self.worker.debug_step(),
            Action::DebugContinue => self.worker.debug_continue(),
            Action::Library => self.view = View::Library,
            Action::Settings => self.view = View::Settings,
            Action::Network => self.view = View::Network,
            Action::Measurements => self.view = View::Measurements,
            Action::Export => self.export(),
        }
    }
    fn top(&mut self, ctx: &egui::Context) {
        ctx.style_mut(|s| {
            s.interaction.resize_grab_radius_side = 8.;
            s.interaction.resize_grab_radius_corner = 14.;
        });
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(crate::i18n::t("RF"))
                        .size(20. * crate::theme::scale())
                        .strong()
                        .color(teal()),
                );
                ui.label(
                    RichText::new(crate::i18n::t("WORKBENCH"))
                        .size(16. * crate::theme::scale())
                        .strong(),
                );
                ui.label(
                    RichText::new(crate::i18n::t("/ Instrumentation & métrologie"))
                        .size(12. * crate::theme::scale())
                        .color(muted()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Accueil / projets").clicked() && !self.worker.is_busy() {
                        self.welcome = true;
                    }
                    badge(ui, concat!("v", env!("CARGO_PKG_VERSION")), muted());
                    badge(
                        ui,
                        if self.hardware {
                            "MATÉRIEL ACTIVÉ"
                        } else {
                            "SIMULATION"
                        },
                        if self.hardware { gold() } else { teal() },
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
                    (View::Dsp, Icon::Network, "RF / DSP"),
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
                    ui.add_space(2.);
                }
                ui.menu_button(t("Vues RF"), |ui| {
                    for view in [
                        View::Waterfall,
                        View::Constellation,
                        View::Smith,
                        View::Eye,
                        View::Timing,
                    ] {
                        if ui.button(view.label()).clicked() {
                            self.view = view;
                            ui.close();
                        }
                    }
                });
                for view in [View::Debug, View::Studio, View::Help] {
                    if ui
                        .selectable_label(self.view == view, view.label())
                        .clicked()
                    {
                        self.view = view;
                    }
                }
            });
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
                    ui.checkbox(&mut self.continuous, crate::i18n::t("Continu"));
                });
                ui.add_enabled_ui(true, |ui| {
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
                    .checkbox(
                        &mut self.canvas.orthogonal,
                        crate::i18n::t("Câbles orthogonaux"),
                    )
                    .changed()
                {
                    self.preferences_draft.orthogonal = self.canvas.orthogonal;
                }
                ui.add_enabled_ui(!busy, |ui| {
                    ui.checkbox(&mut self.hardware, crate::i18n::t("Matériel réel"));
                });
            });
            ui.horizontal_wrapped(|ui| {
                self.project_chip(ui);
                self.project_tools(ui);
                ui.menu_button(t("Édition"), |ui| {
                    for action in [
                        Action::SelectAll,
                        Action::Copy,
                        Action::Paste,
                        Action::Annotate,
                        Action::AutoLayout,
                        Action::Route,
                    ] {
                        if ui.button(t(action.label())).clicked() {
                            self.action(action);
                            ui.close();
                        }
                    }
                    ui.separator();
                    for (label, mode) in [
                        ("Aligner à gauche", crate::editor::Alignment::Left),
                        ("Aligner en haut", crate::editor::Alignment::Top),
                        (
                            "Distribuer horizontalement",
                            crate::editor::Alignment::Horizontal,
                        ),
                        (
                            "Distribuer verticalement",
                            crate::editor::Alignment::Vertical,
                        ),
                    ] {
                        if ui.button(t(label)).clicked() {
                            self.history.record(self.project.graph.clone());
                            crate::editor::align(
                                &mut self.project.graph,
                                &self.canvas.selected_ids,
                                mode,
                            );
                            self.canvas.fit();
                            ui.close();
                        }
                    }
                });
            });
            ui.add_space(3.);
        });
    }
    fn sidebar(&mut self, ctx: &egui::Context) {
        let shown = egui::SidePanel::left(egui::Id::new(("library", self.layout_revision)))
            .resizable(true)
            .default_width(self.layout.library_width)
            .width_range(150. ..=600.)
            .show(ctx, |ui| {
                caption(ui, "BIBLIOTHÈQUE DE BLOCS");
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text(pair("Recherche floue…", "Fuzzy search…"))
                        .desired_width(f32::INFINITY),
                );
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let hits = crate::editor::search(&self.search);
                    if !self.search.trim().is_empty() {
                        caption(ui, "Résultats");
                        for k in hits {
                            self.palette_item(ui, k);
                        }
                    } else {
                        for (name, list) in [
                            ("Favoris", self.studio.favorites.clone()),
                            ("Récents", self.studio.recent.clone()),
                        ] {
                            egui::CollapsingHeader::new(t(name))
                                .default_open(true)
                                .show(ui, |ui| {
                                    for k in list {
                                        self.palette_item(ui, k);
                                    }
                                });
                        }
                        for category in [
                            "Instruments RF",
                            "Alimentations DC",
                            "Électronique & conversion",
                            "Thermique",
                            "Composants DUT",
                            "Analyse & automatisation",
                            "Sources / HAL",
                            "DSP / canal",
                            "Modulation / synchronisation",
                            "Codage canal",
                            "Mesures DSP",
                            "Calibration / unités",
                        ] {
                            egui::CollapsingHeader::new(t(category))
                                .default_open(true)
                                .show(ui, |ui| {
                                    for k in
                                        Kind::ALL.into_iter().filter(|k| k.category() == category)
                                    {
                                        self.palette_item(ui, k);
                                    }
                                });
                        }
                    }
                });
            });
        self.layout.library_width = shown.response.rect.width().clamp(150., 600.);
    }
    fn inspector(&mut self, ctx: &egui::Context) {
        let mut configure = None;
        let shown=egui::SidePanel::right(egui::Id::new(("inspector",self.layout_revision))).resizable(true).default_width(self.layout.inspector_width).width_range(220. ..=850.).show(ctx,|ui|{egui::ScrollArea::vertical().show(ui,|ui|{
            ui.add_space(10.);caption(ui,"INSPECTEUR");
            if let Some(note_id)=self.canvas.annotation {
                let before=self.project.graph.clone();
                if let Some(note)=self.project.graph.annotations.iter_mut().find(|a|a.id==note_id) {
                    caption(ui,"Annotation");
                    if ui.add(egui::TextEdit::multiline(&mut note.text).char_limit(2048).desired_rows(5).desired_width(f32::INFINITY)).changed() {self.history.record(before);}
                }
            }
            ui.label(format!("{} {}",self.canvas.selected_ids.iter().filter(|id|self.project.graph.node(**id).is_some()).count(),pair("bloc(s) sélectionné(s)","block(s) selected")));
            let mut old=None;let mut changed=false;let mut remove=None;let mut open_python=false;
            if let Some(id)=self.canvas.selected && let Some(n)=self.project.graph.nodes.iter_mut().find(|n|n.id==id){
                old=Some(n.clone());badge(ui,n.kind.tag(),crate::theme::kind(n.kind));changed|=ui.text_edit_singleline(&mut n.title).changed();
                let (r,_)=ui.allocate_exact_size(egui::vec2(ui.available_width(),90.),egui::Sense::hover());visuals::symbol_with_ports(ui.painter(),r.shrink(8.),n.kind,n.config.instrument.port_count);ui.separator();let c=&mut n.config;
                if instrument::supports(n.kind) {
                    if n.kind!=Kind::Dut {ui.label(&c.resource);}
                    if n.kind.max_rf_ports().is_some() {ui.label(format!("{} ports RF · {} · Channel {}",c.instrument.port_count,c.s_parameter,c.instrument.channel));}
                    if n.kind==Kind::Dut {ui.label(format!("Gain {:.1} dB · NF {:.1} dB",-c.loss_db-c.attenuation_db,c.noise_figure_db));}
                    ui.label("Double-clic sur le bloc pour ouvrir ses réglages.");
                    if ui.button("Réglages du bloc…").clicked(){configure=Some(id);}
                    if n.kind.is_dc_supply() {ui.label(format!("CH{} · {:.2} V · limite {:.3} A",c.instrument.dc.channel,c.instrument.dc.voltage_v,c.instrument.dc.current_limit_a));}
                }else{match n.kind {
                    Kind::Dsp(op)=>{changed|=dsp::settings_ui(ui,op,&mut c.dsp,id,&mut self.dsp_draft);},
                    Kind::Generator=>{changed|=number(ui,"FRÉQUENCE",&mut c.frequency_hz,1e6," Hz");changed|=number(ui,"PUISSANCE",&mut c.power_dbm,0.1," dBm");},
                    Kind::Dut=>{changed|=number(ui,"PERTE D'INSERTION",&mut c.loss_db,0.1," dB");changed|=number(ui,"FACTEUR DE BRUIT",&mut c.noise_figure_db,0.1," dB");ui.label(c.dut_id.as_deref().unwrap_or("DUT générique · modèle local"));if ui.button(crate::i18n::t("Ouvrir le catalogue DUT")).clicked(){self.view=View::DutCatalog;}ui.label(RichText::new(crate::i18n::t("MODEL fournit un modèle au PNA/NF Meter. RF IN/OUT représente la chaîne de signal.")).size(11. * crate::theme::scale()).color(muted()));},
                    Kind::Analyzer|Kind::Pna|Kind::PnaX|Kind::UsbVna=>{changed|=number(ui,"DÉBUT BALAYAGE",&mut c.start_hz,1e6," Hz");changed|=number(ui,"FIN BALAYAGE",&mut c.stop_hz,1e6," Hz");caption(ui,"POINTS");changed|=ui.add(egui::DragValue::new(&mut c.points).range(2..=rf_core::MAX_POINTS)).changed();},
                    Kind::DcSupplyE3631A|Kind::DcSupplyE36313A=>{ui.label("Réglages dans la fenêtre du bloc.");},
                    Kind::Awg|Kind::Dac|Kind::Adc=>{changed|=number(ui,"CADENCE AWG",&mut c.sample_rate_hz,1e6," Sa/s");changed|=number(ui,"FRÉQUENCE DE BASE",&mut c.tone_hz,1e4," Hz");caption(ui,"ÉCHANTILLONS AWG");changed|=ui.add(egui::DragValue::new(&mut c.samples).range(2..=65536)).changed();caption(ui,"RÉSOLUTION");changed|=ui.add(egui::DragValue::new(&mut c.resolution_bits).range(2..=24).suffix(" bits")).changed();changed|=number(ui,"PLEINE ÉCHELLE",&mut c.voltage_v,0.05," V");},
                    Kind::IqModulator=>{changed|=number(ui,"PERTE CONVERSION",&mut c.loss_db,0.1," dB");ui.label(crate::i18n::t("Entrées I / Q analogiques et oscillateur LO. Modèle idéal de transposition."));},
                    Kind::VariableResistor=>{changed|=number(ui,"RÉSISTANCE",&mut c.resistance_ohm,1.," Ω");},
                    Kind::Thermometer|Kind::Thermostream=>{changed|=number(ui,"TEMPÉRATURE / CONSIGNE",&mut c.temperature_c,0.1," °C");ui.label(crate::i18n::t("Thermostream : consigne idéale sans dynamique thermique. Thermomètre : entrée TEMP ou valeur locale."));},
                    Kind::NoiseFigureMeter=>{changed|=number(ui,"FACTEUR DE BRUIT",&mut c.noise_figure_db,0.1," dB");},
                    Kind::PowerSensor|Kind::PowerMeter=>{ui.label(crate::i18n::t("Mesure scalaire en dBm. Relier RF → Power Sensor → Power Meter."));},
                    Kind::Python=>{if ui.button(crate::i18n::t("Éditer le script Python")).clicked(){self.script=c.script.clone();open_python=true;}ui.label(crate::i18n::t("Entrée trace / sortie output"));},Kind::Peak=>{ui.label(crate::i18n::t("Maximum de trace en dBm"));},
                    Kind::Limit=>{changed|=number(ui,"LIMITE BASSE",&mut c.lower_dbm,0.1," dBm");changed|=number(ui,"LIMITE HAUTE",&mut c.upper_dbm,0.1," dBm");},
                }
                }
                if n.kind.is_extended() && !instrument::supports(n.kind){ui.label(RichText::new(crate::i18n::t("Modèle simulé dans v0.3. Les profils matériels spécifiques restent à développer.")).size(11. * crate::theme::scale()).color(gold()));}
                ui.separator();
                changed|=ui.checkbox(&mut n.breakpoint,crate::i18n::t("Breakpoint")).changed();
                changed|=ui.checkbox(&mut n.probe,crate::i18n::t("Sonde")).changed();
                caption(ui,"Commentaire");
                changed|=ui.add(egui::TextEdit::multiline(&mut n.comment).char_limit(2048).desired_rows(3).desired_width(f32::INFINITY)).changed();
                if ui.button(crate::i18n::t("Aide du bloc")).clicked(){self.help_kind=Some(n.kind);self.view=View::Help;}
                ui.separator();caption(ui,"PORTS DE DONNÉES");for p in n.kind.inputs(){ui.colored_label(port(p.port),format!("IN {} · {}{}",p.name,p.port.label(),if p.required{" · requis"}else{" · optionnel"}));}for p in n.kind.outputs(){ui.colored_label(port(p.port),format!("OUT {} · {}",p.name,p.port.label()));}
                if ui.button(RichText::new(crate::i18n::t("Supprimer le bloc")).color(red())).clicked(){remove=Some(id);}
            }else{ui.label(crate::i18n::t("Sélectionner un bloc."));}
            if changed && let Some(old)=old{let mut before=self.project.graph.clone();if let Some(n)=before.nodes.iter_mut().find(|n|n.id==old.id){*n=old;}self.history.record(before);self.canvas.completed.clear();}
            if let Some(id)=remove{self.history.record(self.project.graph.clone());self.project.graph.remove(id);self.canvas.selected=None;self.canvas.cancel_wire();}
if open_python{self.view=View::Python;}
        });});
        self.layout.inspector_width = shown.response.rect.width().clamp(220., 850.);
        if let Some(id) = configure {
            self.open_instrument(id);
        }
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
                        teal(),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{} blocs  ·  {} câbles",
                            self.project.graph.nodes.len(),
                            self.project.graph.edges.len()
                                + self.project.graph.physical_connections.len()
                        ))
                        .size(11. * crate::theme::scale())
                        .color(muted()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "CPU interface {:.2} ms  ·  cycle {:.1} ms  ·  #{}",
                                self.frame_ms, self.elapsed, self.sequence
                            ))
                            .monospace()
                            .size(11. * crate::theme::scale())
                            .color(muted()),
                        );
                    });
                });
            });
        if !self.preferences.show_journal {
            return;
        }
        let shown = egui::TopBottomPanel::bottom(egui::Id::new(("journal", self.layout_revision)))
            .resizable(true)
            .default_height(self.layout.journal_height)
            .height_range(45. ..=600.)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    caption(ui, "JOURNAL D'EXÉCUTION");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button(crate::i18n::t("Effacer")).clicked() {
                            self.logs.clear();
                        }
                    });
                });
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for (msg, error) in &self.logs {
                            ui.label(
                                RichText::new(t(msg))
                                    .monospace()
                                    .size(11. * crate::theme::scale())
                                    .color(if *error { red() } else { muted() }),
                            );
                        }
                    });
            });
        self.layout.journal_height = shown.response.rect.height().clamp(45., 600.);
    }
    fn center(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(panel()).inner_margin(8.))
            .show(ctx, |ui| {
                // The setup name belongs to its tab; keep the canvas unobstructed.
                self.pane_header(ui, self.view);
                self.render_view(ui, self.view);
            });
    }
    fn render_view(&mut self, ui: &mut egui::Ui, view: View) {
        match view {
            View::Schematic => {
                let height = ui.available_height().max(75.);
                if let Some(error) =
                    self.canvas
                        .show(ui, &mut self.project.graph, &mut self.history, height)
                {
                    self.log(error, true);
                }
            }
            View::Acquisition => {
                if self.trace_display == TraceDisplay::Unavailable {
                    ui.label(pair(
                        "Aucun spectre acquis par ce banc.",
                        "This bench has no acquired spectrum.",
                    ));
                    return;
                }
                self.trace_header(ui);
                if let Ok((freq, power)) = self.trace.peak() {
                    ui.horizontal(|ui| {
                        metric(ui, "PIC", &format!("{power:.3} dBm"), teal());
                        metric(ui, "FRÉQUENCE", &format!("{:.6} GHz", freq / 1e9), blue());
                        metric(
                            ui,
                            "POINTS",
                            &self.trace.amplitude_dbm.len().to_string(),
                            text_color(),
                        );
                    });
                }
                plot::plot(
                    ui,
                    &self.trace,
                    (ui.available_height() - 45.).clamp(120., 700.),
                    self.marker,
                );
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t(
                        "La courbe suit la dernière acquisition complète.",
                    ));
                    if ui.button(crate::i18n::t("Exporter CSV")).clicked() {
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
            View::Waterfall | View::Constellation | View::Smith | View::Eye | View::Timing => {
                self.analysis_view(ui, view)
            }
            View::Debug => self.debug_view(ui),
            View::Studio => self.studio_view(ui),
            View::Help => self.help_view(ui),
            View::Dsp => self.dsp_view(ui),
        }
    }
    fn trace_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            caption(ui, "SPECTRE RF");
            if self.trace_display == TraceDisplay::Preview {
                badge(ui, "APERÇU SIMULÉ", gold());
            }
            ui.label(
                RichText::new(crate::i18n::t("Amplitude / fréquence"))
                    .size(11. * crate::theme::scale())
                    .color(muted()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(&mut self.marker, crate::i18n::t("Marqueur M1"));
            });
        });
    }
    fn test_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.worker.is_busy(),
                    egui::Button::new(crate::i18n::t("Lancer les autotests")),
                )
                .clicked()
            {
                self.submit(Command::Suite);
            }
            if ui
                .add_enabled(
                    !self.worker.is_busy(),
                    egui::Button::new(crate::i18n::t("Tester le banc")),
                )
                .clicked()
            {
                self.run();
            }
        });
        ui.label(RichText::new(crate::i18n::t("Les autotests vérifient le logiciel. Tester le banc évalue les limites définies par ses blocs.")).color(muted()));
        ui.add_space(12.);
        if self.tests.is_empty() {
            ui.label(crate::i18n::t("Aucun résultat. Lancer une série de tests."));
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for t in &self.tests {
                egui::Frame::new()
                    .fill(card_fill())
                    .inner_margin(16.)
                    .corner_radius(8.)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            badge(
                                ui,
                                if t.passed { "PASS" } else { "FAIL" },
                                if t.passed { teal() } else { red() },
                            );
                            ui.strong(&t.name);
                        });
                        ui.label(RichText::new(&t.detail).color(muted()));
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
        ui.label(RichText::new(crate::i18n::t("API : trace, output, ResourceManager().open_resource(...).query(...) / .write(...)")).size(12. * crate::theme::scale()).color(muted()));
        ui.horizontal(|ui| {
            if ui.button(crate::i18n::t("Charger l'exemple")).clicked() {
                self.script = include_str!("../../../python/examples/cable_compensation.py").into();
            }
            if ui
                .button(crate::i18n::t("Appliquer au bloc sélectionné"))
                .clicked()
            {
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
                    egui::Button::new(crate::i18n::t("Exécuter sur la trace")),
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
        ui.label(RichText::new(crate::i18n::t("Le processus est supervisé et interrompu après 5 s (hors requête instrument déjà en cours). Python dispose des permissions de ton compte ; exécuter uniquement des scripts de confiance.")).size(12. * crate::theme::scale()).color(muted()));
    }
    fn instrument_view(&mut self, ui: &mut egui::Ui) {
        caption(ui, "GESTIONNAIRE DE RESSOURCES");
        ui.label(crate::i18n::t(
            "Simulation disponible ; saisir explicitement l'adresse d'un instrument TCP.",
        ));
        ui.add_space(8.);
        ui.label(crate::i18n::t("Ressource"));
        ui.add(egui::TextEdit::singleline(&mut self.resource).desired_width(f32::INFINITY));
        ui.label(crate::i18n::t("Commande SCPI (query)"));
        ui.add(egui::TextEdit::singleline(&mut self.query).desired_width(f32::INFINITY));
        if ui
            .add_enabled(
                !self.worker.is_busy(),
                egui::Button::new(crate::i18n::t("Interroger l'instrument")),
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
                ui.strong(crate::i18n::t("Transport"));
                ui.strong(crate::i18n::t("État"));
                ui.end_row();
                ui.label(crate::i18n::t("Simulation SCPI"));
                badge(ui, "Disponible", teal());
                ui.end_row();
                ui.label(crate::i18n::t("SCPI TCP / SOCKET"));
                badge(ui, "Disponible · timeout 2 s", blue());
                ui.end_row();
                ui.label(crate::i18n::t("USB / GPIB / série via VISA"));
                badge(ui, "Adaptateur · runtime constructeur requis", muted());
                ui.end_row();
                ui.label(crate::i18n::t("VXI-11 / HiSLIP"));
                badge(ui, "Via runtime VISA", muted());
                ui.end_row();
            });
        ui.add_space(16.);
        ui.label(RichText::new(crate::i18n::t("Le pilote d'analyseur initial utilise un profil SCPI générique et une trace ASCII en dBm. Vérifier les commandes dans le manuel du modèle. La découverte réseau automatique et les pilotes constructeur ne sont pas encore intégrés.")).color(muted()));
    }
}
fn metric(ui: &mut egui::Ui, label: &str, value: &str, color: egui::Color32) {
    egui::Frame::new()
        .fill(card_fill())
        .inner_margin(14.)
        .corner_radius(8.)
        .show(ui, |ui| {
            ui.set_min_width(160.);
            caption(ui, label);
            ui.label(
                RichText::new(value)
                    .size(24. * crate::theme::scale())
                    .strong()
                    .color(color),
            );
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
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.worker.stop();
        let _ = self.recover_project();
        let mut preferences = self.studio.clone();
        preferences.workspaces.clear();
        preferences.active = 0;
        if let Ok(text) = serde_json::to_string_pretty(&preferences) {
            let _ = atomic_save(&self.studio_path, &text);
        }
    }

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
        let typing = crate::shortcuts::text_editing(ctx)
            || self.welcome
            || self.instrument_dialog.is_some()
            || self.setup_rename.is_some()
            || self.project_dialog.is_some();
        let native_paste = events.iter().any(|e| matches!(e, egui::Event::Paste(_)));
        for event in &events {
            if !typing {
                match event {
                    egui::Event::Copy => self.copy_selection(),
                    egui::Event::Cut => {
                        self.copy_selection();
                        self.delete_selection();
                    }
                    egui::Event::Paste(text) => self.paste_text(text),
                    _ => {}
                }
            }
            if let Some(action) = self.preferences.action(event, typing)
                && !(action == Action::Paste && native_paste)
            {
                self.action(action);
            }
        }
        crate::i18n::set(self.studio.language);
        let appearance = (
            self.studio.light,
            self.studio.high_contrast,
            self.studio.text_scale,
            self.studio.language,
        );
        if self.appearance != Some(appearance) {
            crate::theme::apply(ctx, appearance.0, appearance.1, appearance.2);
            self.appearance = Some(appearance);
        }
        self.canvas.auto_route = self.studio.auto_route;
        if self.welcome {
            self.welcome_view(ctx);
            self.project_window(ctx);
        } else {
            self.top(ctx);
            self.bottom(ctx);
            self.sidebar(ctx);
            if self.studio.inspector
                && (self.view.dockable()
                    || self.layout.panes.iter().any(|p| p.view == View::Schematic))
            {
                self.inspector(ctx);
            }
            self.docked(ctx);
            self.center(ctx);
            self.floating(ctx);
            self.pna_result_windows(ctx);
            self.flow_window(ctx);
            self.result_manager(ctx);
            self.poll_fixture();
            self.instrument_window(ctx);
            self.fixture_window(ctx);
            self.rename_setup_window(ctx);
            self.project_window(ctx);
        }
        if self.canvas.routes_dirty && self.routing_job.is_none() {
            self.canvas.routes_dirty = false;
            self.start_routing(false, true, false);
        }
        self.layout.canvas_pan = [self.canvas.pan.x, self.canvas.pan.y];
        self.layout.canvas_zoom = self.canvas.zoom;
        if self.view.dockable() {
            self.layout.primary = self.view;
        }
        if let Some(id) = self.canvas.configure_request.take() {
            if self
                .project
                .graph
                .node(id)
                .is_some_and(|n| instrument::supports(n.kind))
            {
                self.open_instrument(id);
            } else {
                self.canvas.select_only(id);
                self.studio.inspector = true;
            }
        }
        if let Some(kind) = self.canvas.help_request.take() {
            self.help_kind = Some(kind);
            self.view = View::Help;
        }
        self.frame_ms = self.frame_ms * 0.9 + start.elapsed().as_secs_f64() * 1000. * 0.1;
        self.last_frame = Instant::now();
        if self.capture_path.is_some() {
            self.capture_frames += 1;
            let ready = (self.project_dialog.is_none() || !self.file_browser.busy())
                && self.routing_job.is_none()
                && (!self.worker.is_busy()
                    || self.debug_snapshot.as_ref().is_some_and(|s| s.paused));
            if self.capture_frames >= 5 && !self.capture_requested && ready {
                self.capture_requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            ctx.request_repaint_after(Duration::from_millis(30));
        }
        if self.worker.is_busy() || self.routing_job.is_some() {
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
