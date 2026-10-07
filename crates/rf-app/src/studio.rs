use crate::{app::View, canvas::History};
use rf_core::{Kind, Measurement, NetworkTrace, Project, Trace, Waveform};
use rf_runtime::debug::Buffer;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    French,
    English,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Location {
    Main,
    Right,
    Bottom,
    Floating,
    #[default]
    Hidden,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pane {
    pub view: View,
    pub location: Location,
    pub rect: [f32; 4],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub library_width: f32,
    pub inspector_width: f32,
    pub journal_height: f32,
    pub primary: View,
    pub panes: Vec<Pane>,
    pub right_width: f32,
    pub bottom_height: f32,
    pub right_active: Option<View>,
    pub bottom_active: Option<View>,
}
impl Default for Layout {
    fn default() -> Self {
        Self {
            library_width: 210.,
            inspector_width: 300.,
            journal_height: 100.,
            primary: View::Schematic,
            panes: vec![Pane {
                view: View::Acquisition,
                location: Location::Bottom,
                rect: [350., 250., 650., 400.],
            }],
            right_width: 360.,
            bottom_height: 250.,
            right_active: None,
            bottom_active: None,
        }
    }
}
impl Layout {
    pub fn set(&mut self, view: View, location: Location) {
        let rect = self
            .panes
            .iter()
            .find(|p| p.view == view)
            .map(|p| p.rect)
            .unwrap_or([350., 250., 650., 400.]);
        self.panes.retain(|p| p.view != view);
        if location == Location::Main {
            self.primary = view;
        } else if location != Location::Hidden {
            self.panes.push(Pane {
                view,
                location,
                rect,
            });
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if !self.library_width.is_finite()
            || !(150. ..=600.).contains(&self.library_width)
            || !self.inspector_width.is_finite()
            || !(220. ..=850.).contains(&self.inspector_width)
            || !self.journal_height.is_finite()
            || !(45. ..=600.).contains(&self.journal_height)
            || self.panes.len() > 12
            || !self.right_width.is_finite()
            || !(240. ..=900.).contains(&self.right_width)
            || !self.bottom_height.is_finite()
            || !(130. ..=700.).contains(&self.bottom_height)
        {
            return Err("Dimensions de layout invalides".into());
        }
        let mut views = std::collections::BTreeSet::new();
        for p in &self.panes {
            if !p.view.dockable()
                || matches!(p.location, Location::Main | Location::Hidden)
                || !views.insert(format!("{:?}", p.view))
                || p.rect.iter().any(|v| !v.is_finite() || v.abs() > 20000.)
                || p.rect[2] < 100.
                || p.rect[3] < 100.
            {
                return Err("Vue dockée invalide ou dupliquée".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub name: String,
    pub project: Project,
    #[serde(default)]
    pub project_path: String,
    #[serde(default)]
    pub csv_path: String,
    pub layout: Layout,
    pub pan: [f32; 2],
    pub zoom: f32,
    #[serde(skip)]
    pub history: History,
    #[serde(skip)]
    pub buffers: Vec<Buffer>,
    #[serde(skip)]
    pub trace: Option<Trace>,
    #[serde(skip)]
    pub network: Option<NetworkTrace>,
    #[serde(skip)]
    pub waveform: Option<Waveform>,
    #[serde(skip)]
    pub measurements: Vec<Measurement>,
    #[serde(skip)]
    pub waterfall: crate::analysis::Waterfall,
}
impl Workspace {
    pub fn new(name: String, project: Project) -> Self {
        Self {
            name,
            project,
            project_path: String::new(),
            csv_path: String::new(),
            layout: Layout::default(),
            pan: [12., -24.],
            zoom: 1.,
            history: History::default(),
            buffers: Vec::new(),
            trace: None,
            network: None,
            waveform: None,
            measurements: Vec::new(),
            waterfall: crate::analysis::Waterfall::default(),
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct SavedLayout {
    pub name: String,
    pub layout: Layout,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Studio {
    pub schema_version: u32,
    pub language: Language,
    pub light: bool,
    pub high_contrast: bool,
    pub text_scale: f32,
    pub auto_route: bool,
    pub inspector: bool,
    pub favorites: Vec<Kind>,
    pub recent: Vec<Kind>,
    pub layouts: Vec<SavedLayout>,
    pub workspaces: Vec<Workspace>,
    pub active: usize,
}
impl Default for Studio {
    fn default() -> Self {
        Self {
            schema_version: 1,
            language: Language::French,
            light: false,
            high_contrast: false,
            text_scale: 1.,
            auto_route: true,
            inspector: true,
            favorites: vec![Kind::Generator, Kind::Analyzer, Kind::PnaX],
            recent: Vec::new(),
            layouts: Vec::new(),
            workspaces: Vec::new(),
            active: 0,
        }
    }
}
impl Studio {
    pub fn used(&mut self, kind: Kind) {
        self.recent.retain(|k| *k != kind);
        self.recent.insert(0, kind);
        self.recent.truncate(8);
    }
    pub fn favorite(&mut self, kind: Kind) {
        if self.favorites.contains(&kind) {
            self.favorites.retain(|k| *k != kind);
        } else {
            self.favorites.push(kind);
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1
            || self.workspaces.len() > 8
            || (!self.workspaces.is_empty() && self.active >= self.workspaces.len())
            || !self.text_scale.is_finite()
            || !(0.85..=1.5).contains(&self.text_scale)
            || self.favorites.len() > Kind::ALL.len()
            || self.recent.len() > 8
            || self.layouts.len() > 20
        {
            return Err("Préférences Studio invalides".into());
        }
        for w in &self.workspaces {
            if w.name.trim().is_empty()
                || w.name.len() > 128
                || !w.zoom.is_finite()
                || !(0.2..=2.).contains(&w.zoom)
                || w.pan.iter().any(|p| !p.is_finite() || p.abs() > 2e6)
            {
                return Err("Espace de travail invalide".into());
            }
            w.layout.validate()?;
            Project::from_json(&w.project.to_json().map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        }
        for l in &self.layouts {
            if l.name.trim().is_empty() || l.name.len() > 128 {
                return Err("Nom de layout invalide".into());
            }
            l.layout.validate()?;
        }
        Ok(())
    }
    pub fn from_json(text: &str) -> Result<Self, String> {
        if text.len() > 32_000_000 {
            return Err("Studio trop volumineux".into());
        }
        let s: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        s.validate()?;
        Ok(s)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workspace_roundtrip_and_layout_moves_keep_one_pane() {
        let mut s = Studio::default();
        let mut w = Workspace::new("Banc A".into(), Project::default());
        w.layout.primary = View::Smith;
        w.project_path = "bench-a.rfw.json".into();
        w.layout.set(View::Smith, Location::Floating);
        w.layout.set(View::Smith, Location::Right);
        assert_eq!(
            w.layout
                .panes
                .iter()
                .filter(|p| p.view == View::Smith)
                .count(),
            1
        );
        s.workspaces.push(w);
        s.language = Language::English;
        s.light = true;
        let text = serde_json::to_string(&s).unwrap();
        let loaded = Studio::from_json(&text).unwrap();
        assert!(loaded.light);
        assert_eq!(loaded.language, Language::English);
        assert_eq!(loaded.workspaces[0].layout.primary, View::Smith);
        assert_eq!(loaded.workspaces[0].project_path, "bench-a.rfw.json");
        assert_eq!(
            loaded.workspaces[0].project.graph,
            s.workspaces[0].project.graph
        );
    }
    #[test]
    fn malformed_layout_and_workspace_indices_are_rejected() {
        let mut s = Studio::default();
        s.workspaces
            .push(Workspace::new("Banc".into(), Project::default()));
        s.active = 3;
        assert!(s.validate().is_err());
        s.active = 0;
        s.workspaces[0].layout.right_width = f32::NAN;
        assert!(s.validate().is_err());
    }
}
