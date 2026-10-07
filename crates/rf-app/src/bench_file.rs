//! Portable bench envelope. Physical I/O authorization is never restored.
use crate::studio::Layout;
use rf_core::Project;
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Clone)]
pub struct BenchFile {
    pub format: String,
    pub version: u32,
    pub project: Project,
    pub layout: Layout,
}
impl BenchFile {
    pub fn new(project: Project, layout: Layout) -> Self {
        Self {
            format: "rf-workbench/bench".into(),
            version: 2,
            project,
            layout,
        }
    }
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > 4_000_000 {
            return Err("Projet trop volumineux".into());
        }
        let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if value.get("format").is_none() {
            return Ok(Self::new(
                Project::from_json(text).map_err(|e| e.to_string())?,
                Layout::default(),
            ));
        }
        let b: Self = serde_json::from_value(value).map_err(|e| e.to_string())?;
        if b.format != "rf-workbench/bench" || !matches!(b.version, 1 | 2) {
            return Err("Format ou version de banc non pris en charge".into());
        }
        Project::from_json(&b.project.to_json().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        b.layout.validate()?;
        for n in &b.project.graph.nodes {
            n.config.instrument.validate()?;
        }
        Ok(b)
    }
    pub fn json(&self) -> Result<String, String> {
        self.layout.validate()?;
        Project::from_json(&self.project.to_json().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        if text.len() > 4_000_000 {
            return Err("Projet > 4 Mo ; réduire le nombre de points des fixtures".into());
        }
        Ok(text)
    }
}
pub fn project_path(path: &str) -> String {
    let p = std::path::Path::new(path);
    if p.extension()
        .is_some_and(|x| x.eq_ignore_ascii_case("rfbench"))
    {
        path.into()
    } else {
        p.with_extension("rfbench").to_string_lossy().into_owned()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bench_and_legacy_roundtrip() {
        let l = Layout {
            library_width: 450.,
            ..Default::default()
        };
        let p = Project::default();
        let b = BenchFile::parse(&BenchFile::new(p.clone(), l).json().unwrap()).unwrap();
        assert_eq!(b.project, p);
        assert_eq!(b.layout.library_width, 450.);
        assert_eq!(BenchFile::parse(&p.to_json().unwrap()).unwrap().project, p);
    }
    #[test]
    fn rejects_bad_layout_version_and_name() {
        let b = BenchFile::new(Project::default(), Layout::default());
        let mut v = serde_json::to_value(b).unwrap();
        v["version"] = serde_json::json!(3);
        assert!(BenchFile::parse(&v.to_string()).is_err());
        v["version"] = serde_json::json!(1);
        v["layout"]["library_width"] = serde_json::json!(0);
        assert!(BenchFile::parse(&v.to_string()).is_err());
        assert!(project_path("banc.json").ends_with("banc.rfbench"));
    }
    #[test]
    fn version_two_preserves_fixtures_and_independent_result_views() {
        let mut project = Project {
            name: "Projet A".into(),
            graph: rf_core::Graph::network_demo(),
            ..Project::default()
        };
        project.graph.nodes[1]
            .config
            .instrument
            .pna
            .fixture
            .input_s2p = "# Hz S RI R 50\n".into();
        let mut layout = Layout::default();
        layout.result_windows.push(crate::results::Window {
            id: 1,
            node: 2,
            parameter: "S11".into(),
            format: crate::results::Format::Smith,
            rect: [300., 200., 500., 300.],
            corrected: true,
        });
        let file = BenchFile::new(project, layout);
        let json = file.json().unwrap();
        let mut a = BenchFile::parse(&json).unwrap();
        let b = BenchFile::parse(&json).unwrap();
        a.project.name = "Projet B".into();
        a.project.graph.nodes[1].config.power_dbm = -40.;
        assert_eq!(b.project.name, "Projet A");
        assert_eq!(b.version, 2);
        assert!(b.layout.result_windows[0].corrected);
        assert!(
            b.project.graph.nodes[1]
                .config
                .instrument
                .pna
                .fixture
                .input_s2p
                .starts_with('#')
        );
        let old = json.replace("\"version\": 2", "\"version\": 1");
        assert!(BenchFile::parse(&old).is_ok());
    }
}
