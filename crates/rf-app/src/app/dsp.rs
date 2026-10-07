use super::*;
use rf_core::dsp::{Data, Op, Settings};
use rf_runtime::debug::BufferData;
use std::collections::BTreeMap;
#[derive(Default)]
pub(super) struct History {
    rows: BTreeMap<u64, VecDeque<rf_core::dsp::Spectrum>>,
}
impl History {
    pub(super) fn push(&mut self, buffers: &[Buffer]) {
        for b in buffers {
            if let BufferData::Dsp(Data::Spectrum(s)) = b.data.as_ref() {
                let rows = self.rows.entry(b.node).or_default();
                if rows
                    .front()
                    .is_some_and(|x| x.frequency_hz != s.frequency_hz || x.unit != s.unit)
                {
                    rows.clear();
                }
                rows.push_front(s.clone());
                rows.truncate(64);
            }
        }
        self.rows
            .retain(|id, _| buffers.iter().any(|b| b.node == *id));
    }
}
#[derive(Default)]
pub(super) struct Draft {
    node: u64,
    text: String,
    error: String,
}
pub(super) fn settings_ui(
    ui: &mut egui::Ui,
    op: Op,
    c: &mut Settings,
    id: u64,
    draft: &mut Draft,
) -> bool {
    let mut changed = false;
    caption(ui, op.category());
    if matches!(
        op,
        Op::DigitalMod | Op::DigitalDemod | Op::AnalogMod | Op::AnalogDemod
    ) {
        egui::ComboBox::from_id_salt("modulation")
            .selected_text(&c.modulation)
            .show_ui(ui, |ui| {
                let list: &[&str] = if matches!(op, Op::AnalogMod | Op::AnalogDemod) {
                    &["AM", "FM", "PM"]
                } else {
                    &["ASK", "FSK", "PSK", "QAM", "OFDM"]
                };
                for m in list {
                    changed |= ui
                        .selectable_value(&mut c.modulation, (*m).into(), *m)
                        .changed();
                }
            });
        caption(ui, "Ordre / samples par symbole");
        changed |= ui
            .add(egui::DragValue::new(&mut c.order).range(2..=256))
            .changed();
        changed |= ui
            .add(egui::DragValue::new(&mut c.sps).range(1..=128))
            .changed();
    }
    if matches!(
        op,
        Op::IqSource | Op::BitSource | Op::IoSource | Op::DigitalMod
    ) {
        changed |= super::number(ui, "Cadence", &mut c.rate, 100., " sample/s");
        caption(ui, "Échantillons / bits");
        changed |= ui
            .add(egui::DragValue::new(&mut c.samples).range(8..=65536))
            .changed();
        changed |= super::number(ui, "Fréquence centrale", &mut c.center_hz, 1e6, " Hz");
    }
    if matches!(op, Op::IqSource | Op::DigitalMod) {
        caption(ui, "Amplitude / unité de source");
        egui::ComboBox::from_id_salt("source-unit")
            .selected_text(c.source_unit.label())
            .show_ui(ui, |ui| {
                for unit in [rf_core::dsp::Unit::Volt, rf_core::dsp::Unit::Fs] {
                    changed |= ui
                        .selectable_value(&mut c.source_unit, unit, unit.label())
                        .changed();
                }
            });
        changed |= super::number(
            ui,
            "Amplitude",
            &mut c.amplitude,
            0.01,
            c.source_unit.label(),
        );
        if c.source_unit == rf_core::dsp::Unit::Fs {
            ui.label("Sortie SDR/audio : |I+jQ| <= 1. En QAM, réduire A (ex. 0.5) pour laisser de la marge.");
        }
    }
    if matches!(op, Op::Psd | Op::Fft | Op::Spectrogram | Op::PhaseNoise) {
        caption(ui, "Taille FFT (puissance de 2)");
        changed |= ui
            .add(egui::DragValue::new(&mut c.fft_size).range(8..=8192))
            .changed();
        egui::ComboBox::from_id_salt("window")
            .selected_text(&c.window)
            .show_ui(ui, |ui| {
                for w in ["Rectangular", "Hann", "Hamming", "Blackman"] {
                    changed |= ui.selectable_value(&mut c.window, w.into(), w).changed();
                }
            });
    }
    if op == Op::Channel {
        changed |= super::number(ui, "SNR AWGN", &mut c.noise_db, 0.1, " dB");
        changed |= ui
            .checkbox(&mut c.fading, "Fading Rayleigh par trame")
            .changed();
        changed |= super::number(ui, "Doppler / offset", &mut c.frequency_offset, 1., " Hz");
        changed |= super::number(ui, "Non-linéarité", &mut c.nonlinearity, 0.01, "");
    }
    if op == Op::Convert {
        egui::ComboBox::from_id_salt("target-unit")
            .selected_text(c.target_unit.label())
            .show_ui(ui, |ui| {
                for unit in rf_core::dsp::Unit::ALL {
                    changed |= ui
                        .selectable_value(&mut c.target_unit, unit, unit.label())
                        .changed();
                }
            });
    }
    if op == Op::CalibrationTable {
        ui.label("volts_per_fs : facteur RMS explicite pour calibrer des données FS en V. null conserve FS.");
    }
    if matches!(op, Op::Decimate | Op::Interpolate) {
        caption(ui, "Facteur");
        changed |= ui
            .add(egui::DragValue::new(&mut c.factor).range(1..=64))
            .changed();
        ui.label("Définir les taps du filtre anti-alias dans les paramètres avancés.");
    }
    if matches!(op, Op::IoSource | Op::IoSink) {
        egui::ComboBox::from_id_salt("hal-backend")
            .selected_text(&c.io_backend)
            .show_ui(ui, |ui| {
                for b in [
                    "RAW", "VISA", "SERIAL", "TCP", "UDP", "SOAPY", "UHD", "IIO", "ZMQ", "AUDIO",
                    "HDF5", "PARQUET",
                ] {
                    changed |= ui
                        .selectable_value(&mut c.io_backend, b.into(), b)
                        .changed();
                }
            });
        caption(ui, "Ressource / chemin / adresse");
        changed |= ui.text_edit_singleline(&mut c.endpoint).changed();
        caption(ui, "Timeout");
        changed |= ui
            .add(
                egui::DragValue::new(&mut c.timeout_ms)
                    .range(1..=30000)
                    .suffix(" ms"),
            )
            .changed();
        if matches!(c.io_backend.as_str(), "VISA" | "SERIAL") {
            caption(ui, "Requête I/Q binaire SCPI");
            changed |= ui.text_edit_singleline(&mut c.scpi_query).changed();
            ui.label("Format cf32 little endian requis. Cadence et fréquence déclarées par le profil ; aucune calibration en volts implicite.");
        } else if matches!(
            c.io_backend.as_str(),
            "SOAPY" | "UHD" | "IIO" | "ZMQ" | "AUDIO" | "HDF5" | "PARQUET"
        ) {
            caption(ui, "Interpréteur avec SDK optionnel");
            changed |= ui.text_edit_singleline(&mut c.python_executable).changed();
        }
        ui.label("L'horloge, le trigger et le MIMO sont contrôlés par les capacités du pilote.");
    }
    if matches!(
        op,
        Op::ConvEncode
            | Op::ConvDecode
            | Op::LdpcEncode
            | Op::LdpcDecode
            | Op::TurboEncode
            | Op::TurboDecode
            | Op::RsEncode
            | Op::RsDecode
    ) {
        ui.colored_label(gold(),"Profils de recherche documentés : K=3 (7,5), RS GF256, LDPC (128,64), Turbo RSC rate 1/3. Aucun profil DVB/3GPP implicite.");
    }
    if id != draft.node || changed {
        *draft = Draft {
            node: id,
            text: serde_json::to_string_pretty(c).unwrap_or_default(),
            error: String::new(),
        };
    }
    egui::CollapsingHeader::new("Paramètres avancés JSON").show(ui, |ui| {
        ui.add(
            egui::TextEdit::multiline(&mut draft.text)
                .code_editor()
                .desired_rows(12)
                .desired_width(f32::INFINITY),
        );
        if ui.button("Appliquer les paramètres").clicked() {
            match serde_json::from_str::<Settings>(&draft.text) {
                Ok(next) => match next.validate() {
                    Ok(()) => {
                        *c = next;
                        changed = true;
                        draft.error.clear();
                    }
                    Err(e) => {
                        draft.error = e;
                    }
                },
                Err(e) => {
                    draft.error = e.to_string();
                }
            }
        }
    });
    if !draft.error.is_empty() {
        ui.colored_label(red(), &draft.error);
    }
    if let Err(e) = c.validate() {
        draft.error = e;
    }
    changed
}
pub(super) fn inspect(ui: &mut egui::Ui, d: &Data) {
    match d {
        Data::Iq(f) => {
            ui.monospace(format!("{} pts · {} · {} sample/s\nfc={} Hz · simulated={}\nfirst={} · epoch={:?}\nclock={} · gap={}",f.samples.len(),f.unit.label(),f.sample_rate,f.center_hz,f.simulated,f.time.first_sample,f.time.epoch_ns,f.time.clock_domain,f.time.discontinuity));
            egui::ScrollArea::vertical()
                .id_salt("iq-debug")
                .max_height(200.)
                .show_rows(ui, 20., f.samples.len(), |ui, range| {
                    for i in range {
                        ui.monospace(format!(
                            "[{i:04}] I={:.7} Q={:.7}",
                            f.samples[i].re, f.samples[i].im
                        ));
                    }
                });
        }
        Data::Bits(f) => {
            ui.monospace(format!(
                "{} bits · simulated={}\nLLR : positif=0, négatif=1",
                f.bits.len(),
                f.simulated
            ));
            ui.monospace(
                f.bits
                    .iter()
                    .take(256)
                    .map(|b| char::from(b'0' + b))
                    .collect::<String>(),
            );
        }
        Data::Spectrum(s) => {
            ui.monospace(format!(
                "{} bins · {} · simulated={}",
                s.levels.len(),
                s.unit.label(),
                s.simulated
            ));
        }
        Data::Quantity {
            value,
            unit,
            simulated,
        } => {
            ui.monospace(format!(
                "{value:.8} {} · simulated={simulated}",
                unit.label()
            ));
        }
    }
}
impl Workbench {
    pub(super) fn dsp_view(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("dsp-dashboard").show(ui,|ui|{
        ui.horizontal_wrapped(|ui|{badge(ui,"BASEBANDE COMPLEXE",purple());if ui.button("Charger QAM16 / AWGN / Viterbi").clicked(){self.replace_demo(rf_runtime::dsp_demo(),"RF / DSP · QAM16 & canal");self.view=View::Dsp;self.dsp_history=History::default();}
 if ui.button("Découvrir GPIB / USB / VISA").clicked(){self.submit(Command::Discover);}});
        ui.label("Sources typées · DSP · canal · démodulation · mesures. Les courbes proviennent des buffers exécutés.");
        ui.horizontal_wrapped(|ui|{egui::ComboBox::from_id_salt("iq-source-dsp").selected_text(self.dsp_iq.and_then(|id|self.buffers.iter().find(|b|b.node==id)).map(|b|b.name.as_str()).unwrap_or("Choisir I/Q")).show_ui(ui,|ui|for b in &self.buffers{if matches!(b.data.as_ref(),BufferData::Dsp(Data::Iq(_))){ui.selectable_value(&mut self.dsp_iq,Some(b.node),&b.name);}});egui::ComboBox::from_id_salt("spectrum-source-dsp").selected_text(self.dsp_spectrum.and_then(|id|self.buffers.iter().find(|b|b.node==id)).map(|b|b.name.as_str()).unwrap_or("Choisir spectre")).show_ui(ui,|ui|for b in &self.buffers{if matches!(b.data.as_ref(),BufferData::Dsp(Data::Spectrum(_))){ui.selectable_value(&mut self.dsp_spectrum,Some(b.node),&b.name);}});});
        if self.dsp_iq.is_none_or(|id|!self.buffers.iter().any(|b|b.node==id)){self.dsp_iq=self.buffers.iter().rev().find(|b|matches!(b.data.as_ref(),BufferData::Dsp(Data::Iq(_)))).map(|b|b.node);}
        if self.dsp_spectrum.is_none_or(|id|!self.buffers.iter().any(|b|b.node==id)){self.dsp_spectrum=self.buffers.iter().find(|b|matches!(b.data.as_ref(),BufferData::Dsp(Data::Spectrum(_)))).map(|b|b.node);}
        ui.horizontal_wrapped(|ui|for m in &self.measurements{egui::Frame::group(ui.style()).show(ui,|ui|{caption(ui,&m.name);ui.label(RichText::new(format!("{:.4} {}",m.value,m.unit)).size(23.).color(teal()));ui.small(if m.simulated{"Simulation"}else{"Acquisition"});});});
        ui.horizontal(|ui|{ui.selectable_value(&mut self.dsp_tab,0,"I/Q & constellation");ui.selectable_value(&mut self.dsp_tab,1,"Spectre & waterfall");});
        if self.dsp_tab==0 {
        if let Some(f)=self.buffers.iter().find(|b|Some(b.node)==self.dsp_iq).and_then(|b|if let BufferData::Dsp(Data::Iq(f))=b.data.as_ref(){Some(f)}else{None}){
            ui.columns(2,|columns|{columns[0].strong("Enveloppe I / Q");let x:Vec<_>=(0..f.samples.len()).map(|i|i as f64/f.sample_rate*1000.).collect();let re:Vec<_>=f.samples.iter().map(|z|z.re).collect();let im:Vec<_>=f.samples.iter().map(|z|z.im).collect();plot::series(&mut columns[0],&x,&re,105.,"ms",f.unit.label(),1.);plot::series(&mut columns[0],&x,&im,105.,"ms",f.unit.label(),1.);columns[1].strong("Constellation / trajectoire I/Q");{let i=Waveform{samples:re,sample_rate_hz:f.sample_rate,tone_hz:0.,unit:f.unit.label().into(),simulated:f.simulated};let q=Waveform{samples:im,..i.clone()};columns[1].allocate_ui(egui::vec2(columns[1].available_width(),210.),|ui|crate::analysis::constellation(ui,&i,&q,1,0));};});
            ui.small(format!("{} · Fs={} sample/s · fc={} Hz · first={} · clock={} · gap={}",f.unit.label(),f.sample_rate,f.center_hz,f.time.first_sample,f.time.clock_domain,f.time.discontinuity));
        }
        }else {
        if let Some(s)=self.buffers.iter().find(|b|Some(b.node)==self.dsp_spectrum).and_then(|b|if let BufferData::Dsp(Data::Spectrum(s))=b.data.as_ref(){Some(s)}else{None}){ui.strong("FFT / densité spectrale");plot::series(ui,&s.frequency_hz,&s.levels,210.,"MHz",s.unit.label(),1e6);}
        if let Some(rows)=self.dsp_spectrum.and_then(|id|self.dsp_history.rows.get(&id)){ui.strong("Spectrogramme / waterfall · acquisitions récentes en haut");let n=rows.front().map_or(0,|s|s.levels.len().min(512));if n>0{let (rect,_)=ui.allocate_exact_size(egui::vec2(ui.available_width(),140.),egui::Sense::hover());let min=rows.iter().flat_map(|s|&s.levels).copied().fold(f64::INFINITY,f64::min);let max=rows.iter().flat_map(|s|&s.levels).copied().fold(f64::NEG_INFINITY,f64::max);let p=ui.painter_at(rect);for (r,s) in rows.iter().enumerate(){for i in 0..n{let v=s.levels[i*s.levels.len()/n];let t=((v-min)/(max-min).max(1e-12)).clamp(0.,1.)as f32;let color=egui::Color32::from_rgb((30.+220.*t)as u8,(35.+185.*t)as u8,(90.+50.*t)as u8);p.rect_filled(egui::Rect::from_min_max(rect.min+egui::vec2(i as f32*rect.width()/n as f32,r as f32*rect.height()/rows.len()as f32),rect.min+egui::vec2((i+1)as f32*rect.width()/n as f32,(r+1)as f32*rect.height()/rows.len()as f32)),0.,color);}}ui.small(format!("{} acquisitions · {:.1} à {:.1} {}",rows.len(),min,max,rows[0].unit.label()));}}
        }
        if self.buffers.is_empty(){ui.label("Charger le banc DSP puis Exécuter (F5).");}
        egui::CollapsingHeader::new("HAL / capacités / ressources").show(ui,|ui|{ui.label("VISA : GPIB, USB, Serial ASRL, LAN INSTR via runtime fabricant. TCP/UDP natifs. SDR/audio/ZMQ/HDF5/Parquet : SDK Python optionnels. PTP/GPSDO/MIMO : contrats disponibles, activation requiert un pilote validé.");for r in &self.resources{if ui.selectable_label(self.resource==*r,r).clicked(){self.resource=r.clone();}}ui.label("Profils : générateur, analyseur, VNA, oscilloscope, power meter et SDR (rf-hal). Aucun pilote par modèle n'est validé sur matériel physique.");});
        });
    }
}
