use super::*;
use rf_instruments::{discovery::Device, pna::Capabilities};
#[derive(Clone)]
pub(super) struct Dialog {
    pub application: Option<rf_instruments::pna_application::ApplicationData>,
    pub id: u64,
    pub kind: Kind,
    pub config: rf_core::Config,
    pub title: String,
    pub tab: u8,
    pub capabilities: Option<Capabilities>,
    pub devices: Vec<Device>,
    pub command: String,
    pub output: String,
    pub binary_path: String,
    pub message: String,
}
pub(super) fn supports(k: Kind) -> bool {
    matches!(
        k,
        Kind::Dut
            | Kind::UsbVna
            | Kind::DcSupplyE3631A
            | Kind::DcSupplyE36313A
            | Kind::Generator
            | Kind::Analyzer
            | Kind::Pna
            | Kind::PnaX
            | Kind::Awg
            | Kind::Adc
            | Kind::Dac
            | Kind::IqModulator
            | Kind::Thermometer
            | Kind::Thermostream
            | Kind::VariableResistor
            | Kind::PowerSensor
            | Kind::PowerMeter
            | Kind::NoiseFigureMeter
    )
}
impl Dialog {
    pub fn new(n: &rf_core::Node, data_directory: &std::path::Path) -> Self {
        Self {
            application: None,
            id: n.id,
            kind: n.kind,
            config: n.config.clone(),
            title: n.title.clone(),
            tab: if n.kind == Kind::Dut { 1 } else { 0 },
            capabilities: None,
            devices: Vec::new(),
            command: "*IDN?".into(),
            output: String::new(),
            binary_path: data_directory
                .join("instrument-data.bin")
                .to_string_lossy()
                .into_owned(),
            message: String::new(),
        }
    }
    pub fn accept_devices(&mut self, devices: Vec<Device>) {
        let matching: Vec<_> = devices
            .iter()
            .filter(|d| {
                d.error.is_none()
                    && if !self.config.instrument.expected_idn.trim().is_empty() {
                        d.idn
                            .to_lowercase()
                            .contains(&self.config.instrument.expected_idn.trim().to_lowercase())
                    } else if matches!(self.kind, Kind::Pna | Kind::PnaX) {
                        rf_instruments::pna::is_pna(&d.idn)
                    } else if self.kind == Kind::UsbVna {
                        rf_instruments::pna::is_usb_vna(&d.idn)
                    } else if self.kind.is_dc_supply() {
                        rf_instruments::dc::identifies(self.kind, &d.idn)
                    } else {
                        false
                    }
            })
            .collect();
        if matching.len() == 1 && self.config.resource == "SIM::RF::INSTR" {
            self.config.resource = matching[0].resource.clone();
            self.message =
                "Un appareil correspondant identifié : adresse proposée. Appliquer pour la mémoriser.".into();
        }
        self.devices = devices;
    }
}
impl Workbench {
    pub(super) fn open_instrument(&mut self, id: u64) {
        if let Some(n) = self.project.graph.node(id) {
            self.instrument_dialog = Some(Dialog::new(n, self.data_directory.path()));
        }
    }
    pub(super) fn instrument_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.instrument_dialog.take() else {
            return;
        };
        let mut open = true;
        let mut close = false;
        let mut apply = false;
        let mut job = None;
        let previous_port_count = d.config.instrument.port_count;
        egui::Window::new(format!("{} · réglages du bloc",d.title)).id(egui::Id::new(("instrument",d.id))).open(&mut open).resizable(true).min_size([470.,340.]).default_pos([340.,100.]).default_size([820.,620.]).max_height((ctx.content_rect().height()-140.).max(350.)).show(ctx,|ui|{
            ui.horizontal_wrapped(|ui|{for (i,label) in ["Connexion","Fonctions","Options PNA","Console SCPI"].iter().enumerate(){if (d.kind!=Kind::Dut || i==1) && (i!=2 || matches!(d.kind,Kind::Pna|Kind::PnaX)) && ui.selectable_label(d.tab==i as u8,*label).clicked(){d.tab=i as u8;}}});ui.separator();
            egui::ScrollArea::vertical().id_salt("instrument-content").max_height((ui.available_height()-70.).max(200.)).show(ui,|ui|{
                let c=&mut d.config;
                match d.tab {
                    0=>{
                        ui.heading("Adresse et identification");
                        ui.label("IDN attendu (fragment modèle ou numéro de série, facultatif)");ui.text_edit_singleline(&mut c.instrument.expected_idn);ui.label("VISA : GPIB / USB / LAN / ASRL ; socket SCPI TCP natif.");
                        ui.add(egui::TextEdit::singleline(&mut c.resource).hint_text("GPIB0::16::INSTR / TCPIP0::192.168.1.10::inst0::INSTR").desired_width(f32::INFINITY));
                        ui.horizontal_wrapped(|ui|{ui.label("Timeout");ui.add(egui::DragValue::new(&mut c.instrument.timeout_ms).range(1..=30000).suffix(" ms"));
                            if ui.add_enabled(!self.worker.is_busy(),egui::Button::new("Détecter VISA + identifier")).clicked(){job=Some(Command::IdentifyInstruments {node:d.id,manual:c.resource.clone(),hardware:self.hardware,timeout_ms:c.instrument.timeout_ms});}
                            if matches!(d.kind,Kind::Pna|Kind::PnaX) && ui.add_enabled(!self.worker.is_busy()&&self.hardware,egui::Button::new("Lire canaux / options PNA")).clicked(){job=Some(Command::InspectPna{node:d.id,config:c.clone(),hardware:self.hardware});}
                        });
                        ui.label("La liste dépend du runtime VISA. Un appareil LAN non découvert peut être ajouté par son adresse ou son nom DNS. Aucun scan de sous-réseau.");
                        for device in &d.devices{if ui.selectable_label(c.resource==device.resource,format!("{}  |  {}",device.resource,device.error.as_deref().unwrap_or(&device.idn))).clicked(){c.resource=device.resource.clone();d.capabilities=None;}}
                        ui.separator();ui.label(if self.hardware{"Mode Matériel activé : les boutons de connexion contactent l'équipement."}else{"Mode Simulation : activer Matériel dans la barre d'outils pour identifier un équipement réel."});
                    }
                    1=>{
                        ui.heading("Réglages du bloc");
                        match d.kind {
                            Kind::Pna|Kind::PnaX|Kind::UsbVna=>{
                                ui.horizontal(|ui|{ui.label("Ports RF du schéma");egui::ComboBox::from_id_salt("vna-ports").selected_text(format!("{} port(s)",c.instrument.port_count)).show_ui(ui,|ui|{for count in 1..=d.kind.max_rf_ports().unwrap(){ui.selectable_value(&mut c.instrument.port_count,count,format!("{count} port(s)"));}});});
                                ui.small("Mise à jour immédiate des broches. Réduire ce nombre retire les câbles des ports supprimés. Le canal de mesure reste indépendant.");
                                if d.kind==Kind::UsbVna {ui.label("Pilote binaire Keysight P50xx/P93xx : le connecteur USB ne définit pas un dialecte SCPI universel.");}
                                if let Some(cap)=&d.capabilities && let Some(count)=cap.port_count {ui.label(format!("Équipement identifié : {count} ports physiques"));}
                                ui.horizontal_wrapped(|ui|{ui.label("Channel");if let Some(cap)=d.capabilities.as_ref().filter(|cap|cap.resource==c.resource){egui::ComboBox::from_id_salt("pna-channels").selected_text(format!("{}",c.instrument.channel)).show_ui(ui,|ui|{for ch in &cap.channels{ui.selectable_value(&mut c.instrument.channel,*ch,format!("Channel {ch}"));}});}ui.add(egui::DragValue::new(&mut c.instrument.channel).range(1..=1000));ui.label("Transfert SDATA");egui::ComboBox::from_id_salt("pna-precision").selected_text(format!("REAL{}",c.instrument.precision)).show_ui(ui,|ui|{ui.selectable_value(&mut c.instrument.precision,32,"REAL32 · compact");ui.selectable_value(&mut c.instrument.precision,64,"REAL64");});});
                                ui.label("Nom de mesure instrument (vide : une seule trace correspondant au paramètre S)");ui.text_edit_singleline(&mut c.instrument.measurement);
                                if let Some(cap)=d.capabilities.as_ref().filter(|cap|cap.resource==c.resource&&cap.channel==c.instrument.channel){egui::ComboBox::from_id_salt("pna-measurement").selected_text("Choisir une trace détectée").show_ui(ui,|ui|{for (name,param) in &cap.measurements{if ui.selectable_label(c.instrument.measurement==*name,format!("{name} · {param}")).clicked(){c.instrument.measurement=name.clone();if rf_core::physical::s_parameter_ports(param).is_some_and(|(i,j)|i<=c.instrument.port_count&&j<=c.instrument.port_count){c.s_parameter=param.clone();}}}});}
                                egui::ComboBox::from_id_salt("pna-parameter").selected_text(&c.s_parameter).show_ui(ui,|ui|{for i in 1..=c.instrument.port_count {for j in 1..=c.instrument.port_count {let p=format!("S{i}{j}");ui.selectable_value(&mut c.s_parameter,p.clone(),&p);}}});
                                ui.checkbox(&mut c.instrument.configure_sweep,"Appliquer le balayage avant acquisition");
                                ui.checkbox(&mut c.instrument.trigger,"Déclencher un nouveau balayage et attendre *OPC?");
                                ui.label("Cases désactivées : lecture des dernières données du canal existant. Aucun preset ni changement de calibration/RF.");
                                ui.add_enabled_ui(c.instrument.configure_sweep,|ui|{
                                    number(ui,"Début",&mut c.start_hz,1e6," Hz");number(ui,"Fin",&mut c.stop_hz,1e6," Hz");ui.label("Points");ui.add(egui::DragValue::new(&mut c.points).range(2..=rf_core::MAX_POINTS));number(ui,"IF bandwidth",&mut c.instrument.if_bandwidth_hz,100.," Hz");ui.checkbox(&mut c.instrument.averaging,"Moyennage");ui.add(egui::DragValue::new(&mut c.instrument.averages).range(1..=65536).suffix(" acquisitions"));
                                });
                                ui.label("Axe lu en REAL64 sur l'appareil ; données SDATA réelles/imaginaires en REAL32/64, little endian. Format de transfert restauré après lecture.");
                            }
                            Kind::Dut=>{dut_settings(ui,c,&self.catalog);}
                            Kind::DcSupplyE3631A|Kind::DcSupplyE36313A=>{
                                let dc=&mut c.instrument.dc;
                                egui::ComboBox::from_id_salt("dc-channel").selected_text(format!("CH{}",dc.channel)).show_ui(ui,|ui|{
                                    for channel in 1..=3 {let label=if d.kind==Kind::DcSupplyE3631A {["P6V · 0…6 V / 5 A","P25V · 0…25 V / 1 A","N25V · −25…0 V / 1 A"][(channel-1) as usize]}else{["CH1 · 0…6 V / 10 A","CH2 · 0…25 V / 2 A","CH3 · 0…25 V / 2 A"][(channel-1) as usize]};ui.selectable_value(&mut dc.channel,channel,label);}
                                });
                                number(ui,"Tension",&mut dc.voltage_v,0.01," V");number(ui,"Limite de courant",&mut dc.current_limit_a,0.001," A");
                                if let Err(error)=dc.validate(d.kind){ui.colored_label(red(),error);}
                                ui.label(if d.kind==Kind::DcSupplyE3631A {"ON/OFF est global aux TROIS sorties de l'E3631A ; vérifier les consignes des autres sorties sur l'appareil."}else{"Mode indépendant FIX requis ; les modes série/parallèle et les sorties couplées sont refusés."});
                                ui.horizontal_wrapped(|ui|{for (label,action) in [("Lire V / I / état",rf_instruments::dc::Action::Read),("Appliquer consignes (OFF)",rf_instruments::dc::Action::Apply),("Activer sortie ON",rf_instruments::dc::Action::Enable),("Couper sortie OFF",rf_instruments::dc::Action::Disable)] {
                                    if ui.add_enabled(!self.worker.is_busy(),egui::Button::new(label)).clicked(){job=Some(Command::DcSupply{node:d.id,kind:d.kind,config:c.clone(),action,hardware:self.hardware});}
                                }});
                                ui.label("Exécuter le banc lit V/I sans changer les consignes. Stop coupe les sorties activées par cette session. Simulation : pas de solveur de charge DC.");
                                ui.monospace(&d.output);
                            }
                            Kind::Generator=>{number(ui,"Fréquence",&mut c.frequency_hz,1e6," Hz");number(ui,"Puissance",&mut c.power_dbm,0.1," dBm");ui.label("Le banc configure FREQ/POW et active OUTP ; arrêt RF demandé en fin/Stop. Dialecte générique à valider.");}
                            Kind::Analyzer=>{number(ui,"Début",&mut c.start_hz,1e6," Hz");number(ui,"Fin",&mut c.stop_hz,1e6," Hz");ui.add(egui::DragValue::new(&mut c.points).range(2..=rf_core::MAX_POINTS).suffix(" points"));ui.label("Requête ASCII du banc");ui.text_edit_singleline(&mut c.trace_query);}
                            Kind::Awg|Kind::Adc|Kind::Dac=>{number(ui,"Cadence",&mut c.sample_rate_hz,1e6," sample/s");number(ui,"Tone",&mut c.tone_hz,1e4," Hz");number(ui,"Amplitude pleine échelle",&mut c.voltage_v,0.1," V");ui.add(egui::DragValue::new(&mut c.samples).range(2..=65536).suffix(" échantillons"));ui.add(egui::DragValue::new(&mut c.resolution_bits).range(2..=24).suffix(" bits"));ui.label("Génération / conversion simulée ; transfert AWG constructeur à implémenter. Console SCPI disponible pour un appareil réel.");}
                            Kind::Thermometer|Kind::Thermostream=>{number(ui,"Température / consigne",&mut c.temperature_c,0.1," °C");}
                            Kind::VariableResistor=>{number(ui,"Résistance",&mut c.resistance_ohm,1.," Ω");}
                            Kind::NoiseFigureMeter=>{number(ui,"Facteur de bruit simulé",&mut c.noise_figure_db,0.1," dB");}
                            _=>{number(ui,"Fréquence de référence",&mut c.frequency_hz,1e6," Hz");number(ui,"Perte de conversion",&mut c.loss_db,0.1," dB");}
                        }
                        if !matches!(d.kind,Kind::Pna|Kind::PnaX|Kind::UsbVna|Kind::Generator|Kind::Analyzer|Kind::Dut|Kind::DcSupplyE3631A|Kind::DcSupplyE36313A){ui.separator();ui.label("Commandes du profil (console, pas exécutées automatiquement par le banc)");ui.label("Lecture");ui.text_edit_singleline(&mut c.instrument.read_query);ui.label("Consigne / configuration");ui.text_edit_singleline(&mut c.instrument.set_command);}
                    }
                    2=>{
                        ui.heading("Capacités détectées");
                        if ui.add_enabled(!self.worker.is_busy()&&self.hardware&&d.capabilities.as_ref().is_some_and(|cap|cap.resource==c.resource&&cap.channel==c.instrument.channel&&cap.supports(&cap.class)),egui::Button::new("Lire FDATA binaire du canal / application")).clicked(){job=Some(Command::PnaApplication{node:d.id,config:c.clone(),compression:false,hardware:self.hardware});}
                        if let Some(data)=&d.application {
                            ui.label(format!("{} · {} · {} · format {} · {} points",data.class,data.measurement,data.parameter,data.display_format,data.x.len()));
                            ui.label("Axe X et valeurs Y dans les unités de l'affichage instrument. Cette lecture ne déclenche aucun sweep.");
                            plot::series(ui,&data.x,&data.y,200.,"axe instrument","affichage",1.);
                            if ui.button("Exporter les données JSON (fichier neuf)").clicked(){
                                let value=serde_json::json!({"resource":c.resource,"channel":data.channel,"class":data.class,"measurement":data.measurement,"parameter":data.parameter,"display_format":data.display_format,"x":data.x,"y":data.y,"units":"instrument-display","simulated":false});
                                use std::io::Write;
                                d.message=match std::fs::OpenOptions::new().write(true).create_new(true).open(&d.binary_path).and_then(|mut f|f.write_all(value.to_string().as_bytes())){Ok(())=>format!("Export : {}",d.binary_path),Err(e)=>e.to_string()};
                            }
                            ui.text_edit_singleline(&mut d.binary_path);
                        }
                        if let Some(cap)=d.capabilities.as_ref().filter(|cap|cap.resource==c.resource&&cap.channel==c.instrument.channel){
                            ui.label(&cap.idn);if let Some((min,max))=cap.frequency_range {ui.label(format!("Plage rapportée : {:.3}–{:.3} GHz (calibration et plage spécifiée à vérifier)",min/1e9,max/1e9));}ui.label(format!("Canaux : {:?} ; canal {} : {}",cap.channels,cap.channel,cap.class));ui.label(format!("*OPT? : {}",cap.options));ui.label(format!("Licences VALID : {}",cap.licenses));
                            for class in ["Standard","Gain Compression","Noise Figure Cold Source","Spectrum Analyzer","Swept IMD","IM Spectrum"]{let available=cap.supports(class);ui.horizontal(|ui|{ui.add_enabled(available,egui::Button::new(class));ui.label(if available{"Disponible"}else if cap.valid_classes.is_none(){"Non déterminé"}else{"Non disponible"});});}
                            ui.label("Les boutons indiquent les classes autorisées. Préparer les applications avancées sur l'appareil ; les canaux existants et leurs calibrations sont conservés.");
                            ui.add_enabled_ui(cap.supports("Gain Compression")&&cap.class=="Gain Compression",|ui|{
                                number(ui,"Compression",&mut c.instrument.compression_db,0.1," dB");
                                if ui.add_enabled(!self.worker.is_busy()&&self.hardware,egui::Button::new("Appliquer niveau GCA au canal existant")).clicked(){job=Some(Command::PnaApplication{node:d.id,config:c.clone(),compression:true,hardware:self.hardware});}
                            });
                            for warning in &cap.warnings{ui.colored_label(gold(),warning);}
                        }else{ui.label("Lire canaux / options dans Connexion. Les fonctions inconnues restent désactivées ; aucun déblocage d'après le seul modèle.");}
                        ui.label("GCA / NF / spectre / IMD : la classe validée ne garantit pas la gamme en fréquence, les récepteurs, sources, atténuateurs ni la calibration nécessaires.");
                    }
                    _=>{
                        ui.heading("Console instrument");ui.add(egui::TextEdit::singleline(&mut d.command).desired_width(f32::INFINITY));
                        ui.horizontal_wrapped(|ui|{for (label,command) in [("IDN","*IDN?"),("Options","*OPT?"),("Erreurs","SYST:ERR?")]{if ui.button(label).clicked(){d.command=command.into();}}
if ui.button("Lecture du profil").clicked(){d.command=c.instrument.read_query.clone();}
if ui.button("Consigne du profil").clicked(){d.command=c.instrument.set_command.clone();}});
                        ui.horizontal_wrapped(|ui|{for (label,write) in [("Query texte",false),("Envoyer write",true)]{if ui.add_enabled(!self.worker.is_busy()&&(self.hardware||c.resource=="SIM::RF::INSTR"),egui::Button::new(label)).clicked(){job=Some(Command::InstrumentConsole{node:d.id,config:c.clone(),command:d.command.clone(),write,binary_path:None,hardware:self.hardware});}}});
                        ui.label("Destination du bloc binaire IEEE 488.2 (payload brut, fichier neuf)");ui.text_edit_singleline(&mut d.binary_path);
                        if ui.add_enabled(!self.worker.is_busy()&&self.hardware,egui::Button::new("Query binaire → fichier")).clicked(){job=Some(Command::InstrumentConsole{node:d.id,config:c.clone(),command:d.command.clone(),write:false,binary_path:Some(d.binary_path.clone()),hardware:self.hardware});}
                        ui.label("Les commandes write sont envoyées telles quelles et peuvent modifier l'état/RF. Vérifier le manuel constructeur. Aucune relance automatique.");ui.monospace(&d.output);
                    }
                }
            });
            ui.separator();ui.label(&d.message);ui.horizontal_wrapped(|ui|{apply=ui.add_enabled(!self.worker.is_busy(),egui::Button::new("Appliquer au bloc")).clicked();if ui.button("Fermer").clicked(){close=true;}});
        });
        if previous_port_count != d.config.instrument.port_count {
            let before = self.project.graph.clone();
            match self
                .project
                .graph
                .set_vna_ports(d.id, d.config.instrument.port_count)
            {
                Ok(removed) => {
                    self.history.record(before);
                    d.config.s_parameter = self
                        .project
                        .graph
                        .node(d.id)
                        .unwrap()
                        .config
                        .s_parameter
                        .clone();
                    self.canvas.cancel_wire();
                    self.canvas.routes_dirty = true;
                    d.message = format!("Ports mis à jour ; {removed} câble(s) retiré(s).");
                }
                Err(e) => {
                    d.config.instrument.port_count = previous_port_count;
                    d.message = e.to_string();
                }
            }
        }
        if let Some(job) = job {
            self.submit(job);
        }
        if apply {
            let result = d.config.instrument.validate().and_then(|_| {
                if d.kind.is_dc_supply() {
                    d.config.instrument.dc.validate(d.kind)?;
                }
                rf_instruments::Resource::parse(&d.config.resource)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            });
            match result {
                Ok(()) => {
                    self.history.record(self.project.graph.clone());
                    if let Some(n) = self.project.graph.nodes.iter_mut().find(|n| n.id == d.id) {
                        n.config = d.config.clone();
                        n.title = d.title.clone();
                    }
                    d.message = "Réglages mémorisés dans le bloc.".into();
                }
                Err(e) => d.message = e,
            }
        }
        if open && !close && self.project.graph.node(d.id).is_some() {
            self.instrument_dialog = Some(d);
        }
    }
}
pub(super) fn component_details(ui: &mut egui::Ui, component: &Component) {
    ui.label(&component.description);
    if let Some(rf) = &component.rf {
        ui.label(format!(
            "{:.2}–{:.2} GHz · référence {:.2} GHz · {}",
            rf.band_hz[0] / 1e9,
            rf.band_hz[1] / 1e9,
            rf.reference_frequency_hz / 1e9,
            rf.revision
        ));
        egui::Grid::new(("component-spec", &component.id))
            .striped(true)
            .show(ui, |ui| {
                for header in ["Caractéristique", "Min.", "Typ.", "Max."] {
                    ui.strong(header);
                }
                ui.end_row();
                for (label, spec, unit) in [
                    ("Gain", &rf.gain_db, "dB"),
                    ("NF", &rf.noise_figure_db, "dB"),
                    ("P1dB sortie", &rf.output_p1db_dbm, "dBm"),
                ] {
                    ui.label(label);
                    ui.label(spec.minimum.map_or("—".into(), |v| format!("{v} {unit}")));
                    ui.label(format!("{} {unit}", spec.typical));
                    ui.label(spec.maximum.map_or("—".into(), |v| format!("{v} {unit}")));
                    ui.end_row();
                }
            });
        ui.small(&rf.conditions);
        if rf.attenuation_states > 1 {
            ui.label(format!(
                "Atténuation : {} états, pas {} dB · phase : {} états, pas {}°",
                rf.attenuation_states, rf.attenuation_step_db, rf.phase_states, rf.phase_step_deg
            ));
        }
        for note in &rf.notes {
            ui.small(note);
        }
    }
    for (key, value) in &component.attributes {
        ui.label(format!("{key} : {value}"));
    }
    for source in &component.sources {
        ui.hyperlink_to(
            format!("Fiche constructeur · relevée le {}", source.retrieved_on),
            &source.url,
        );
    }
}
fn dut_settings(ui: &mut egui::Ui, c: &mut rf_core::Config, catalog: &Catalog) {
    let mut gain = -c.loss_db;
    if number(ui, "Gain de référence du modèle", &mut gain, 0.1, " dB") {
        c.loss_db = -gain;
    }
    number(ui, "Facteur de bruit", &mut c.noise_figure_db, 0.1, " dB");
    if let Some(component) = catalog
        .entries
        .iter()
        .find(|e| Some(e.id.as_str()) == c.dut_id.as_deref())
    {
        if let Some(rf) = &component.rf {
            egui::ComboBox::from_id_salt("dut-mode")
                .selected_text(&c.dut_mode)
                .show_ui(ui, |ui| {
                    for mode in &rf.modes {
                        ui.selectable_value(&mut c.dut_mode, mode.clone(), mode);
                    }
                });
            if rf.attenuation_states > 1 {
                let mut state = (c.attenuation_db / rf.attenuation_step_db)
                    .round()
                    .clamp(0., (rf.attenuation_states - 1) as f64)
                    as u8;
                ui.horizontal(|ui| {
                    ui.label("Code atténuation (6 bits)");
                    if ui
                        .add(egui::DragValue::new(&mut state).range(0..=rf.attenuation_states - 1))
                        .changed()
                    {
                        c.attenuation_db = state as f64 * rf.attenuation_step_db;
                    }
                    ui.label(format!("{:.3} dB", c.attenuation_db));
                });
                let mut state = (c.phase_deg / rf.phase_step_deg)
                    .round()
                    .clamp(0., (rf.phase_states - 1) as f64) as u8;
                ui.horizontal(|ui| {
                    ui.label("Code phase (6 bits)");
                    if ui
                        .add(egui::DragValue::new(&mut state).range(0..=rf.phase_states - 1))
                        .changed()
                    {
                        c.phase_deg = state as f64 * rf.phase_step_deg;
                    }
                    ui.label(format!("{:.3}°", c.phase_deg));
                });
                ui.label(format!(
                    "Gain effectif simulé : {:.3} dB",
                    -c.loss_db - c.attenuation_db
                ));
            }
        }
        ui.separator();
        component_details(ui, component);
    } else {
        number(
            ui,
            "Atténuation additionnelle",
            &mut c.attenuation_db,
            0.1,
            " dB",
        );
        number(ui, "Phase additionnelle", &mut c.phase_deg, 1., "°");
        ui.label("Modèle DUT générique. Ajouter un composant documenté depuis le catalogue DUT.");
    }
    ui.separator();
    ui.small("Modèle linéaire à valeurs constantes : P1dB et NF sont documentés, la compression et le bruit I/Q ne sont pas simulés ici. Les commandes de puce SPI ne sont pas envoyées au matériel.");
}
