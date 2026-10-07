//! Per-user writable data, independent of the executable and working directory.
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

pub(crate) struct DataDirectory {
    path: PathBuf,
    warning: Option<String>,
    // Keep temporary fallback data available for the entire application session.
    _temporary: Option<tempfile::TempDir>,
}

impl DataDirectory {
    pub(crate) fn initialize() -> Self {
        prepare(
            candidate(std::env::consts::OS, |key| std::env::var_os(key)),
            &std::env::temp_dir(),
        )
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn file(&self, name: &str) -> String {
        self.path.join(name).to_string_lossy().into_owned()
    }

    pub(crate) fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }
}

fn candidate(platform: &str, env: impl Fn(&str) -> Option<OsString>) -> Result<PathBuf, String> {
    let value = |key| env(key).filter(|v| !v.is_empty()).map(PathBuf::from);
    let path = if let Some(path) = value("RF_WORKBENCH_DATA_DIR") {
        path
    } else {
        let root = match platform {
            "windows" => value("LOCALAPPDATA")
                .or_else(|| value("USERPROFILE").map(|p| p.join("AppData").join("Local"))),
            "macos" => value("HOME").map(|p| p.join("Library").join("Application Support")),
            _ => value("XDG_DATA_HOME")
                .or_else(|| value("HOME").map(|p| p.join(".local").join("share"))),
        };
        root.ok_or("Dossier de données utilisateur introuvable")?
            .join("RF Workbench")
    };
    if !path.is_absolute() {
        return Err("Le dossier de données doit être un chemin absolu (RF_WORKBENCH_DATA_DIR ou dossier utilisateur)".into());
    }
    Ok(path)
}

fn prepare(requested: Result<PathBuf, String>, temporary_root: &Path) -> DataDirectory {
    let primary = requested.and_then(|path| {
        std::fs::create_dir_all(&path)
            .and_then(|()| tempfile::NamedTempFile::new_in(&path).map(drop))
            .map_err(|e| format!("Dossier de données inaccessible : {} : {e}", path.display()))?;
        Ok(path)
    });
    match primary {
        Ok(path) => DataDirectory {
            path,
            warning: None,
            _temporary: None,
        },
        Err(error) => {
            let fallback = tempfile::Builder::new()
                .prefix("rf-workbench-")
                .tempdir_in(temporary_root);
            match fallback {
                Ok(directory) => DataDirectory {
                    path: directory.path().to_path_buf(),
                    warning: Some(format!(
                        "{error}. Les réglages sont temporaires pour cette session ; enregistrer les projets dans un dossier accessible avant de quitter."
                    )),
                    _temporary: Some(directory),
                },
                Err(fallback_error) => DataDirectory {
                    path: temporary_root.join("RF Workbench"),
                    warning: Some(format!(
                        "{error}. Le dossier temporaire est aussi inaccessible : {fallback_error}. L'interface reste utilisable, mais les enregistrements peuvent échouer ; choisir un dossier accessible."
                    )),
                    _temporary: None,
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_data_uses_account_directory_without_cwd() {
        let user = tempfile::tempdir().unwrap();
        let local = user.path().join("LocalAppData");
        let selected = candidate("windows", |key| {
            (key == "LOCALAPPDATA").then(|| local.clone().into_os_string())
        })
        .unwrap();
        assert_eq!(selected, local.join("RF Workbench"));
        let data = prepare(Ok(selected.clone()), user.path());
        assert_eq!(data.path(), selected);
        assert!(data.warning().is_none());
        assert!(Path::new(&data.file("preferences.rfw.json")).is_absolute());
    }

    #[test]
    fn portable_override_is_absolute_and_takes_priority() {
        let user = tempfile::tempdir().unwrap();
        let portable = user.path().join("portable-data");
        let selected = candidate("windows", |key| match key {
            "RF_WORKBENCH_DATA_DIR" => Some(portable.clone().into_os_string()),
            "LOCALAPPDATA" => Some(user.path().join("other").into_os_string()),
            _ => None,
        })
        .unwrap();
        assert_eq!(selected, portable);
        assert!(
            candidate("windows", |key| {
                (key == "RF_WORKBENCH_DATA_DIR").then(|| OsString::from("relative-data"))
            })
            .is_err()
        );
    }

    #[test]
    fn mac_and_linux_use_user_roots() {
        let user = tempfile::tempdir().unwrap();
        let home = |key: &str| (key == "HOME").then(|| user.path().as_os_str().to_owned());
        assert_eq!(
            candidate("macos", home).unwrap(),
            user.path().join("Library/Application Support/RF Workbench")
        );
        assert_eq!(
            candidate("linux", home).unwrap(),
            user.path().join(".local/share/RF Workbench")
        );
    }

    #[test]
    fn inaccessible_data_falls_back_without_changing_existing_files() {
        let user = tempfile::tempdir().unwrap();
        let existing = user.path().join("existing-file");
        std::fs::write(&existing, "preserve").unwrap();
        let data = prepare(Ok(existing.clone()), user.path());
        assert!(data.warning().is_some());
        assert_ne!(data.path(), existing);
        assert!(data.path().is_dir());
        std::fs::write(data.file("bench.rfbench"), "local project").unwrap();
        assert_eq!(std::fs::read_to_string(existing).unwrap(), "preserve");
    }
}
