use crate::{bench_file::BenchFile, studio::Layout};
use rf_core::{Graph, Project};
use std::{io::Write, path::{Path, PathBuf}};

pub struct Draft {
    pub name: String,
    pub suggested_name: String,
    pub directory: String,
    pub graph: Graph,
    pub layout: Layout,
    pub error: String,
    pub folders: bool,
}
impl Draft {
    pub fn new(graph: Graph, suggested: &str, directory: String) -> Self {
        Self { name: String::new(), suggested_name: suggested.into(), directory, graph,
            layout: Layout::default(), error: String::new(), folders: false }
    }
    pub fn destination(&self) -> Result<PathBuf, String> {
        let name = self.name.trim();
        if name.is_empty() || name.len()>128 || name.chars().any(char::is_control) {
            return Err("Choisir un nom de projet non vide (128 octets maximum).".into());
        }
        let stem = name.trim_end_matches(['.', ' ']);
        if stem.is_empty() || name.chars().any(|c| "<>:\"/\\|?*".contains(c)) {
            return Err("Le nom ne peut pas contenir < > : \" / \\ | ? * ni uniquement des points.".into());
        }
        let device = stem.split('.').next().unwrap_or_default().to_ascii_uppercase();
        if ["CON","PRN","AUX","NUL"].contains(&device.as_str())
            || (device.len()==4 && (device.starts_with("COM") || device.starts_with("LPT"))
                && device.ends_with(['1','2','3','4','5','6','7','8','9'])) {
            return Err("Ce nom est réservé par Windows ; choisir un autre nom.".into());
        }
        let directory = Path::new(self.directory.trim());
        if !directory.is_absolute() || !directory.is_dir() {
            return Err("Choisir un dossier existant avec son chemin complet.".into());
        }
        let filename = if stem.to_ascii_lowercase().ends_with(".rfbench") {
            stem.to_owned()
        } else { format!("{stem}.rfbench") };
        Ok(directory.join(filename))
    }
    pub fn create(&self) -> Result<(BenchFile, String), String> {
        let path = self.destination()?;
        let file = BenchFile::new(Project {
            schema_version: rf_core::SCHEMA_VERSION, name: self.name.trim().into(), graph:self.graph.clone()
        }, self.layout.clone());
        let text = file.json()?;
        let mut output = std::fs::OpenOptions::new().write(true).create_new(true).open(&path)
            .map_err(|e| if e.kind()==std::io::ErrorKind::AlreadyExists {
                "Un projet porte déjà ce nom dans ce dossier. Choisir un autre nom ou dossier.".into()
            } else { format!("Création impossible : {e}") })?;
        if let Err(e) = output.write_all(text.as_bytes()).and_then(|_|output.sync_all()) {
            drop(output);
            let _ = std::fs::remove_file(&path);
            return Err(format!("Enregistrement impossible : {e}"));
        }
        Ok((file, path.to_string_lossy().into_owned()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_project_is_saved_before_opening_and_existing_files_are_preserved() {
        let folder=tempfile::tempdir().unwrap();
        let mut d=Draft::new(Graph::default(),"Exemple",folder.path().display().to_string());
        assert!(d.create().is_err());
        d.name="Mon banc RF".into();
        let (file,path)=d.create().unwrap();
        let original=std::fs::read_to_string(&path).unwrap();
        assert_eq!(BenchFile::parse(&original).unwrap().project.name,file.project.name);
        assert!(d.create().err().unwrap().contains("déjà"));
        assert_eq!(std::fs::read_to_string(path).unwrap(),original);
    }
    #[test]
    fn names_cannot_escape_the_selected_directory_or_use_reserved_devices() {
        let folder=tempfile::tempdir().unwrap();
        let mut d=Draft::new(Graph::default(),"",folder.path().display().to_string());
        for name in ["", "  ", "../banc", "CON", "com1.rfbench", "aux.test", "a\\b", "..."] {
            d.name=name.into();assert!(d.destination().is_err(),"{name}");
        }
        d.name="Banc été".into();assert_eq!(d.destination().unwrap().parent(),Some(folder.path()));
    }
}
