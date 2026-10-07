use eframe::egui::{self, Key};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub fn text_editing(ctx: &egui::Context) -> bool {
    ctx.memory(|m| m.focused())
        .is_some_and(|id| egui::text_edit::TextEditState::load(ctx, id).is_some())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Select,
    Wire,
    Pan,
    Fit,
    Run,
    Stop,
    Save,
    Open,
    Undo,
    Redo,
    Duplicate,
    Delete,
    Library,
    Settings,
    Network,
    Measurements,
    Export,
    Copy,
    Paste,
    SelectAll,
    AutoLayout,
    Route,
    Annotate,
    Help,
    BlockHelp,
    DebugStart,
    DebugStep,
    DebugContinue,
}
impl Action {
    pub const ALL: [Self; 28] = [
        Self::Select,
        Self::Wire,
        Self::Pan,
        Self::Fit,
        Self::Run,
        Self::Stop,
        Self::Save,
        Self::Open,
        Self::Undo,
        Self::Redo,
        Self::Duplicate,
        Self::Delete,
        Self::Library,
        Self::Settings,
        Self::Network,
        Self::Measurements,
        Self::Export,
        Self::Copy,
        Self::Paste,
        Self::SelectAll,
        Self::AutoLayout,
        Self::Route,
        Self::Annotate,
        Self::Help,
        Self::BlockHelp,
        Self::DebugStart,
        Self::DebugStep,
        Self::DebugContinue,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Sélection",
            Self::Wire => "Câblage personnalisé",
            Self::Pan => "Déplacement",
            Self::Fit => "Ajuster le schéma",
            Self::Run => "Exécuter",
            Self::Stop => "Arrêter",
            Self::Save => "Enregistrer le projet",
            Self::Open => "Ouvrir le projet",
            Self::Undo => "Annuler",
            Self::Redo => "Rétablir",
            Self::Duplicate => "Dupliquer le bloc",
            Self::Delete => "Supprimer le bloc",
            Self::Library => "Bibliothèque visuelle",
            Self::Settings => "Raccourcis & préférences",
            Self::Network => "Paramètres S",
            Self::Measurements => "Formes d'onde & mesures",
            Self::Export => "Exporter le spectre",
            Self::Copy => "Copier",
            Self::Paste => "Coller",
            Self::SelectAll => "Tout sélectionner",
            Self::AutoLayout => "Organiser le graphe",
            Self::Route => "Router les câbles",
            Self::Annotate => "Ajouter une annotation",
            Self::Help => "Aide globale",
            Self::BlockHelp => "Aide du bloc",
            Self::DebugStart => "Démarrer le debug",
            Self::DebugStep => "Pas à pas",
            Self::DebugContinue => "Continuer",
        }
    }
    fn default_keys(self) -> &'static str {
        match self {
            Self::Select => "V",
            Self::Wire => "W",
            Self::Pan => "H",
            Self::Fit => "F",
            Self::Run => "F5",
            Self::Stop => "Shift+F5",
            Self::Save => "Ctrl+S",
            Self::Open => "Ctrl+O",
            Self::Undo => "Ctrl+Z",
            Self::Redo => "Ctrl+Y,Ctrl+Shift+Z",
            Self::Duplicate => "Ctrl+D",
            Self::Delete => "Delete",
            Self::Library => "B",
            Self::Settings => "Ctrl+K",
            Self::Network => "N",
            Self::Measurements => "M",
            Self::Export => "Ctrl+E",
            Self::Copy => "Ctrl+C",
            Self::Paste => "Ctrl+V",
            Self::SelectAll => "Ctrl+A",
            Self::AutoLayout => "Ctrl+L",
            Self::Route => "Ctrl+R",
            Self::Annotate => "Ctrl+Shift+A",
            Self::Help => "F1",
            Self::BlockHelp => "F2",
            Self::DebugStart => "F6",
            Self::DebugStep => "F10",
            Self::DebugContinue => "F8",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Binding {
    pub action: Action,
    pub keys: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub schema_version: u32,
    pub shortcuts: Vec<Binding>,
    pub snap: bool,
    pub orthogonal: bool,
    pub show_journal: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            schema_version: 1,
            shortcuts: Action::ALL
                .into_iter()
                .map(|action| Binding {
                    action,
                    keys: action.default_keys().into(),
                })
                .collect(),
            snap: true,
            orthogonal: true,
            show_journal: true,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Chord {
    key: Key,
    command: bool,
    alt: bool,
    shift: bool,
}
fn parse(text: &str) -> Result<Chord, String> {
    let mut c = Chord {
        key: Key::Escape,
        command: false,
        alt: false,
        shift: false,
    };
    let mut key = None;
    for part in text.split('+').map(str::trim) {
        match part.to_lowercase().as_str() {
            "ctrl" | "cmd" | "command" => {
                if c.command {
                    return Err("Modificateur répété".into());
                }
                c.command = true;
            }
            "alt" => {
                if c.alt {
                    return Err("Modificateur répété".into());
                }
                c.alt = true;
            }
            "shift" => {
                if c.shift {
                    return Err("Modificateur répété".into());
                }
                c.shift = true;
            }
            _ => {
                if key.is_some() {
                    return Err(format!("Raccourci invalide : {text}"));
                }
                key = Key::ALL
                    .iter()
                    .copied()
                    .find(|k| k.name().eq_ignore_ascii_case(part));
                if key.is_none() {
                    return Err(format!("Touche inconnue : {part}"));
                }
            }
        }
    }
    c.key = key.ok_or("Touche manquante")?;
    if c.key == Key::Escape {
        return Err("Échap est réservé à l'annulation du câble".into());
    }
    Ok(c)
}
impl Preferences {
    pub fn upgrade(mut self) -> Self {
        for action in Action::ALL {
            if !self.shortcuts.iter().any(|b| b.action == action) {
                let keys = action.default_keys();
                let used = self
                    .shortcuts
                    .iter()
                    .flat_map(|b| b.keys.split(','))
                    .any(|s| parse(s.trim()).ok() == parse(keys).ok());
                self.shortcuts.push(Binding {
                    action,
                    keys: if used { String::new() } else { keys.into() },
                });
            }
        }
        self
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 || self.shortcuts.len() != Action::ALL.len() {
            return Err("Version ou liste d'actions invalide".into());
        }
        let mut actions = BTreeSet::new();
        let mut chords = BTreeSet::new();
        for b in &self.shortcuts {
            if !actions.insert(format!("{:?}", b.action)) {
                return Err("Action dupliquée".into());
            }
            if b.keys.len() > 256 {
                return Err("Trop de raccourcis".into());
            }
            for shortcut in b.keys.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                let c = parse(shortcut)?;
                let id = format!("{:?}:{}:{}:{}", c.key, c.command, c.alt, c.shift);
                if !chords.insert(id) {
                    return Err(format!("Conflit de raccourci : {shortcut}"));
                }
            }
        }
        Ok(())
    }
    pub fn action(&self, event: &egui::Event, text_editing: bool) -> Option<Action> {
        if text_editing {
            return None;
        }
        let egui::Event::Key {
            key,
            pressed: true,
            repeat: false,
            modifiers,
            ..
        } = event
        else {
            return None;
        };
        self.shortcuts
            .iter()
            .find(|b| {
                b.keys
                    .split(',')
                    .filter_map(|s| parse(s.trim()).ok())
                    .any(|c| {
                        c.key == *key
                            && c.command == modifiers.command
                            && c.alt == modifiers.alt
                            && c.shift == modifiers.shift
                    })
            })
            .map(|b| b.action)
    }
    pub fn hint(&self, action: Action) -> String {
        self.shortcuts
            .iter()
            .find(|b| b.action == action)
            .map(|b| format!("{} ({})", action.label(), b.keys))
            .unwrap_or_else(|| action.label().into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn focused_buttons_allow_shortcuts_but_text_editors_suspend_them() {
        let ctx = egui::Context::default();
        let mut text = String::new();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let field = ui.text_edit_singleline(&mut text);
                field.request_focus();
                assert!(text_editing(ctx));
                let button = ui.button("Run");
                button.request_focus();
                assert!(ctx.wants_keyboard_input());
                assert!(!text_editing(ctx));
            });
        });
    }
    #[test]
    fn old_bindings_upgrade_without_overwriting_user_shortcuts() {
        let mut p = Preferences::default();
        p.shortcuts.truncate(17);
        p.shortcuts[0].keys = "F1".into();
        let upgraded = p.upgrade();
        upgraded.validate().unwrap();
        assert_eq!(upgraded.shortcuts[0].keys, "F1");
        assert!(
            upgraded
                .shortcuts
                .iter()
                .find(|b| b.action == Action::Help)
                .unwrap()
                .keys
                .is_empty()
        );
        assert_eq!(
            upgraded
                .shortcuts
                .iter()
                .find(|b| b.action == Action::Copy)
                .unwrap()
                .keys,
            "Ctrl+C"
        );
    }
    fn event(key: Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        }
    }
    #[test]
    fn shortcuts_do_not_trigger_in_python_or_text_fields() {
        let p = Preferences::default();
        p.validate().unwrap();
        assert_eq!(
            p.action(&event(Key::W, egui::Modifiers::NONE), false),
            Some(Action::Wire)
        );
        assert_eq!(p.action(&event(Key::W, egui::Modifiers::NONE), true), None);
        assert_eq!(
            p.action(&event(Key::W, egui::Modifiers::SHIFT), false),
            None
        );
    }
    #[test]
    fn aliases_persist_and_conflicts_are_rejected() {
        let mut p = Preferences::default();
        p.shortcuts
            .iter_mut()
            .find(|b| b.action == Action::Wire)
            .unwrap()
            .keys = "W,Ctrl+Shift+W".into();
        p.validate().unwrap();
        let p: Preferences = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(
            p.action(
                &event(
                    Key::W,
                    egui::Modifiers {
                        command: true,
                        ctrl: true,
                        shift: true,
                        ..Default::default()
                    }
                ),
                false
            ),
            Some(Action::Wire)
        );
        let mut p = p;
        p.shortcuts[0].keys = "w".into();
        assert!(p.validate().is_err());
    }
    #[test]
    fn invalid_and_reserved_keys_are_rejected() {
        assert!(parse("Ctrl+").is_err());
        assert!(parse("Foo").is_err());
        assert!(parse("Escape").is_err());
        assert!(parse("Ctrl+Ctrl+W").is_err());
    }
}
