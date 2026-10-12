//! Compact plan editor and interactive functional RF path diagram.
use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};
use rf_core::{
    Config,
    pna_plan::{Arrangement, Format, PathSettings, Plan},
};
use rf_instruments::pna_path::{Element, Snapshot};

pub fn plan_editor(ui: &mut egui::Ui, c: &mut Config) {
    ui.heading("Plan de mesure et panneau PNA");
    ui.small(format!(
        "Channel {} commun aux acquisitions et aux graphiques",
        c.instrument.channel
    ));
    ui.horizontal_wrapped(|ui| {
        if ui.button("Paramètres S · 4 fenêtres").clicked() {
            c.instrument.pna.power_sweep = false;
            c.instrument.pna.all_s_parameters = false;
            c.instrument.pna.plan = Some(Plan::s_parameters(c.instrument.port_count));
            c.s_parameter = "S21".into();
            if c.instrument.port_count == 1 {
                c.s_parameter = "S11".into();
            }
            c.instrument.measurement.clear();
        }
        if ui
            .button("Puissance · gain / réflexion / récepteurs")
            .clicked()
        {
            let source = c.instrument.pna.source_port.min(c.instrument.port_count);
            let receiver = if source == 1 {
                2.min(c.instrument.port_count)
            } else {
                1
            };
            c.instrument.pna.power_sweep = true;
            c.instrument.pna.all_s_parameters = false;
            c.s_parameter = format!("S{receiver}{source}");
            c.instrument.measurement.clear();
            c.instrument.pna.plan = Some(Plan::power(source, receiver));
        }
    });
    let enabled = c.instrument.pna.plan.is_some();
    let mut use_plan = enabled;
    if ui
        .checkbox(
            &mut use_plan,
            "Utiliser ce plan commun (interface, simulation, instrument)",
        )
        .changed()
    {
        c.instrument.pna.plan = use_plan.then(|| Plan::s_parameters(c.instrument.port_count));
    }
    let Some(plan) = &mut c.instrument.pna.plan else {
        return;
    };
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        let mut options = Vec::new();
        for i in 1..=c.instrument.port_count {
            for j in 1..=c.instrument.port_count {
                if !c.instrument.pna.power_sweep || j == c.instrument.pna.source_port {
                    options.push(format!("S{i}{j}"));
                }
            }
        }
        // Receiver names match the PNA catalogue's underscore notation.
        if c.instrument.pna.power_sweep {
            let s = c.instrument.pna.source_port;
            options.push(format!("R{s}_{s}"));
            for i in 1..=c.instrument.port_count {
                options.push(format!("{}_{s}", ["A", "B", "C", "D"][usize::from(i - 1)]));
            }
        }
        for p in options {
            let mut selected = plan.parameters.contains(&p);
            if ui.checkbox(&mut selected, &p).changed() {
                if selected {
                    plan.parameters.push(p);
                } else {
                    plan.parameters.retain(|value| value != &p);
                }
                changed = true;
            }
        }
    });
    if changed {
        let order = |p: &String| match p.as_str() {
            "S11" => 0,
            "S12" => 1,
            "S21" => 2,
            "S22" => 3,
            _ => 4,
        };
        plan.parameters.sort_by_key(order);
        plan.rebuild_windows();
        if !plan.parameters.contains(&c.s_parameter)
            && let Some(p) = plan.parameters.iter().find(|p| p.starts_with('S'))
        {
            c.s_parameter = p.clone();
            c.instrument.measurement.clear();
        }
    }
    ui.horizontal_wrapped(|ui| {
        for (a, label) in [
            (Arrangement::Single, "1 fenêtre"),
            (Arrangement::Two, "2 fenêtres"),
            (Arrangement::Quad, "4 fenêtres · 2 × 2"),
        ] {
            ui.selectable_value(&mut plan.arrangement, a, label);
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui.button("Une trace par fenêtre").clicked() {
            plan.rebuild_windows();
        }
        if ui.button("Superposer les traces").clicked() {
            let traces = plan.windows.iter().flat_map(|w| w.traces.clone()).collect();
            plan.windows = vec![rf_core::pna_plan::DisplayWindow {
                traces,
                active_trace: 0,
            }];
            plan.arrangement = Arrangement::Single;
        }
    });
    for (wi, w) in plan.windows.iter_mut().enumerate() {
        egui::CollapsingHeader::new(format!("Fenêtre {} · {} trace(s)", wi + 1, w.traces.len()))
            .id_salt(("pna-plan-window", wi))
            .show(ui, |ui| {
                for (ti, t) in w.traces.iter_mut().enumerate() {
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(&mut w.active_trace, ti, &t.parameter);
                        egui::ComboBox::from_id_salt(("pna-plan-format", wi, ti))
                            .selected_text(format!("{:?}", t.format))
                            .show_ui(ui, |ui| {
                                for (format, label) in [
                                    (Format::Magnitude, "Module dB"),
                                    (Format::Phase, "Phase °"),
                                    (Format::Smith, "Smith"),
                                    (Format::Raw, "Puissance dBm / valeur Y"),
                                ] {
                                    if t.parameter.starts_with('S') || format == Format::Raw {
                                        ui.selectable_value(&mut t.format, format, label);
                                    }
                                }
                            });
                        ui.checkbox(&mut t.automatic, "Auto");
                    });
                    ui.add_enabled_ui(!t.automatic, |ui| {
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::DragValue::new(&mut t.per_division)
                                    .range(1e-6..=1e6)
                                    .speed(0.1)
                                    .prefix("/div "),
                            );
                            ui.add(
                                egui::DragValue::new(&mut t.reference)
                                    .speed(0.1)
                                    .prefix("Réf "),
                            );
                            ui.add(
                                egui::DragValue::new(&mut t.reference_position)
                                    .range(0. ..=10.)
                                    .speed(0.1)
                                    .prefix("Position "),
                            );
                        });
                    });
                }
            });
    }
    ui.checkbox(
        &mut plan.synchronize_display,
        "Synchroniser l'écran de l'appareil lors de la configuration",
    );
    ui.add(
        egui::DragValue::new(&mut plan.first_window)
            .range(1..=1024)
            .prefix("Première fenêtre instrument "),
    );
    ui.small("Les traces affichées dans ces numéros de fenêtres seront remplacées. Les mesures et calibrations existantes sont conservées. Les autres fenêtres restent présentes ; TILE peut donc afficher plus de quatre fenêtres.");
    if let Err(error) = plan.validate(
        c.instrument.port_count,
        c.instrument
            .pna
            .power_sweep
            .then_some(c.instrument.pna.source_port),
    ) {
        ui.colored_label(Color32::LIGHT_RED, error);
    }
}

fn simulated_elements(ports: u8) -> Vec<Element> {
    let mut entries = vec![
        ("Combiner", "Normal", vec!["Normal", "Reversed"]),
        ("Src1Out1LowBand", "Filtered", vec!["Filtered", "HiPwr"]),
        ("Port1Bypass", "Thru", vec!["Thru", "Combiner"]),
        ("Port2Bypass", "Thru", vec!["Thru", "RearPanel"]),
        ("Port1RefMxr", "Internal", vec!["Internal", "External"]),
        ("Port2Src", "Src1Out2", vec!["Src1Out2", "Src2Out1"]),
        ("Src2Out1LowBand", "Filtered", vec!["Filtered", "HiPwr"]),
    ];
    if ports == 4 {
        entries.extend([
            ("Port3Bypass", "Thru", vec!["Thru", "Combiner"]),
            ("Port4Bypass", "Thru", vec!["Thru", "RearPanel"]),
        ]);
    }
    entries
        .into_iter()
        .map(|(name, current, values)| Element {
            name: name.into(),
            current: current.into(),
            values: values.into_iter().map(str::to_owned).collect(),
        })
        .collect()
}
pub fn path_editor(ui: &mut egui::Ui, c: &mut Config, snapshot: Option<&Snapshot>, hardware: bool) {
    ui.heading("PNA-X · architecture interne et chemins RF / IF");
    let live = snapshot.filter(|s| s.resource == c.resource && s.channel == c.instrument.channel);
    let simulation = simulated_elements(c.instrument.port_count);
    let elements = if hardware {
        live.map_or(&[][..], |s| s.elements.as_slice())
    } else {
        &simulation
    };
    if let Some(s) = live {
        ui.label(&s.idn);
        ui.label(format!(
            "Channel {} · configuration actuelle : {}",
            s.channel, s.name
        ));
        ui.label(&s.description);
        egui::ComboBox::from_id_salt("pna-path-recall")
            .selected_text(if c.instrument.pna.path.configuration.is_empty() {
                "Éléments individuels"
            } else {
                &c.instrument.pna.path.configuration
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut c.instrument.pna.path.configuration,
                    String::new(),
                    "Éléments individuels",
                );
                for name in &s.configurations {
                    ui.selectable_value(
                        &mut c.instrument.pna.path.configuration,
                        name.clone(),
                        name,
                    );
                }
            });
    } else {
        ui.small(if hardware {"Lire le catalogue RF/IF sur l'appareil pour activer les éléments disponibles."}else{"Schéma fonctionnel de référence Opt 423 / 224. Les routages RF internes ne sont pas un modèle calibré de pertes. Les options réelles seront interrogées en mode Matériel."});
    }
    ui.checkbox(
        &mut c.instrument.pna.path.apply,
        "Appliquer les chemins avant les réglages de mesure",
    );
    diagram(
        ui,
        c.instrument.port_count,
        &mut c.instrument.pna.path,
        elements,
    );
    egui::CollapsingHeader::new("Tous les éléments RF / IF disponibles").show(ui, |ui| {
        for e in elements {
            ui.horizontal_wrapped(|ui| {
                ui.label(&e.name);
                let mut override_value = c.instrument.pna.path.elements.contains_key(&e.name);
                if ui.checkbox(&mut override_value, "Modifier").changed() {
                    if override_value {
                        c.instrument
                            .pna
                            .path
                            .elements
                            .insert(e.name.clone(), e.current.clone());
                    } else {
                        c.instrument.pna.path.elements.remove(&e.name);
                    }
                }
                if let Some(value) = c.instrument.pna.path.elements.get_mut(&e.name) {
                    egui::ComboBox::from_id_salt(("path-element", &e.name))
                        .selected_text(value.as_str())
                        .show_ui(ui, |ui| {
                            for v in &e.values {
                                ui.selectable_value(value, v.clone(), v);
                            }
                        });
                } else {
                    ui.weak(&e.current);
                }
            });
        }
    });
    ui.small("Combiner et Port2Src décrivent des jumpers à connecter manuellement. Changer leur dessin ne déplace aucun câble. Rappeler un chemin peut modifier l'IF : fréquence, IFBW, puissance et calibration sont appliquées ensuite.");
    ui.hyperlink_to(
        "Documentation Keysight · Path Configurator",
        "https://helpfiles.keysight.com/csg/NA520xA/S1_Settings/Path_Configurator.htm",
    );
    ui.hyperlink_to(
        "Schéma Keysight · mesures haute puissance",
        "https://helpfiles.keysight.com/csg/NA520xA/Tutorials/High_Power_PNA-X.htm",
    );
}
fn diagram(ui: &mut egui::Ui, ports: u8, path: &mut PathSettings, elements: &[Element]) {
    let width = ui.available_width().max(400.);
    let (area, _) = ui.allocate_exact_size(Vec2::new(width, 390.), Sense::hover());
    let painter = ui.painter_at(area);
    painter.rect_filled(area, 6., Color32::from_rgb(19, 31, 43));
    let order: Vec<u8> = if ports == 4 {
        vec![1, 3, 4, 2]
    } else {
        (1..=ports).collect()
    };
    let cell = width / order.len() as f32;
    let xy = |x: f32, y: f32| area.min + Vec2::new(x, y);
    let source_box = |x: f32, label: &str| {
        let r = Rect::from_center_size(xy(x, 28.), Vec2::new(108., 30.));
        painter.rect_filled(r, 5., Color32::from_rgb(55, 80, 106));
        painter.text(
            r.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(14.),
            Color32::WHITE,
        );
    };
    source_box(width * 0.25, "Source 1");
    if ports > 1 {
        source_box(width * 0.75, "Source 2");
    }
    for (index, port) in order.iter().enumerate() {
        let x = cell * (index as f32 + 0.5);
        let sx = if *port == 1 || *port == 2 {
            width * 0.25
        } else {
            width * 0.75
        };
        let level = if *port == 1 || *port == 3 { 76. } else { 86. };
        painter.add(egui::Shape::line(
            vec![xy(sx, 43.), xy(sx, level), xy(x, level), xy(x, 100.)],
            Stroke::new(1., Color32::from_rgb(105, 189, 154)),
        ));
    }
    painter.text(
        xy(width / 2., 57.),
        Align2::CENTER_CENTER,
        "OUT1 / OUT2 · atténuation source · filtres",
        FontId::proportional(10.),
        Color32::LIGHT_GRAY,
    );
    painter.line_segment(
        [xy(15., 96.), xy(width - 15., 96.)],
        Stroke::new(1., Color32::GRAY),
    );
    let draw_element =
        |ui: &mut egui::Ui, r: Rect, name: &str, label: &str, path: &mut PathSettings| {
            let element = elements.iter().find(|e| e.name == name);
            let value = path
                .elements
                .get(name)
                .map(String::as_str)
                .or_else(|| element.map(|e| e.current.as_str()))
                .unwrap_or("?");
            let color = if element.is_some() {
                Color32::from_rgb(45, 90, 111)
            } else {
                Color32::from_rgb(46, 49, 58)
            };
            painter.rect(
                r,
                4.,
                color,
                Stroke::new(1., Color32::from_gray(125)),
                StrokeKind::Inside,
            );
            painter.text(
                r.center(),
                Align2::CENTER_CENTER,
                format!("{label}\n{value}"),
                FontId::proportional(11.),
                Color32::WHITE,
            );
            let response = ui.interact(r, ui.id().with(("schema-path", name)), Sense::click());
            response.clone().on_hover_text(if element.is_some() {
                format!("{name} · cliquer pour changer de réglage dans le brouillon")
            } else {
                format!("{name} · disponibilité non détectée")
            });
            if response.clicked()
                && let Some(e) = element
            {
                let i = e.values.iter().position(|v| v == value).unwrap_or(0);
                path.elements
                    .insert(name.into(), e.values[(i + 1) % e.values.len()].clone());
            }
        };
    draw_element(
        ui,
        Rect::from_center_size(xy(width / 2., 166.), Vec2::new(100., 28.)),
        "Combiner",
        "Combiner",
        path,
    );
    for (index, port) in order.iter().enumerate() {
        let x = cell * (index as f32 + 0.5);
        painter.line_segment(
            [xy(x, 100.), xy(x, 338.)],
            Stroke::new(2., Color32::from_rgb(105, 189, 154)),
        );
        draw_element(
            ui,
            Rect::from_center_size(xy(x, 130.), Vec2::new((cell - 14.).min(156.), 40.)),
            &format!("Port{port}Bypass"),
            &format!("SW{port} · bypass"),
            path,
        );
        let branch = Rect::from_center_size(xy(x, 205.), Vec2::new((cell - 14.).min(150.), 46.));
        painter.rect_filled(branch, 4., Color32::from_rgb(50, 64, 75));
        painter.text(
            branch.center(),
            Align2::CENTER_CENTER,
            "Coupleur / pont\n+ biais selon options",
            FontId::proportional(11.),
            Color32::WHITE,
        );
        let receiver = ["A", "B", "C", "D"][usize::from(port - 1)];
        for (offset, label) in [(-42., format!("R{port}")), (42., receiver.into())] {
            painter.add(egui::Shape::line(
                vec![xy(x, 227.), xy(x + offset, 239.), xy(x + offset, 297.)],
                Stroke::new(1., Color32::LIGHT_BLUE),
            ));
            let r = Rect::from_center_size(xy(x + offset, 264.), Vec2::new(62., 38.));
            painter.rect_filled(r, 4., Color32::from_rgb(41, 64, 84));
            painter.text(
                r.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(14.),
                Color32::LIGHT_BLUE,
            );
        }
        painter.text(
            xy(x, 310.),
            Align2::CENTER_CENTER,
            "Atténuation / mélangeurs / IF",
            FontId::proportional(10.),
            Color32::LIGHT_BLUE,
        );
        painter.circle_filled(xy(x, 339.), 4., Color32::WHITE);
        painter.text(
            xy(x, 360.),
            Align2::CENTER_CENTER,
            format!("P{port}"),
            FontId::proportional(16.),
            Color32::WHITE,
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn draft_editors_render_without_hardware_capabilities_at_different_sizes() {
        for width in [480., 800., 1200.] {
            let ctx = egui::Context::default();
            let mut c = Config::default();
            c.instrument.port_count = 4;
            c.instrument.pna.plan = Some(Plan::default());
            let frame = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(width, 900.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            plan_editor(ui, &mut c);
                            path_editor(ui, &mut c, None, false);
                        });
                    });
                },
            );
            assert!(!frame.shapes.is_empty());
            assert!(c.instrument.pna.path.elements.is_empty());
            c.instrument.validate().unwrap();
        }
    }
}
