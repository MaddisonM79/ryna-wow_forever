//! The app's own settings, in its config folder (`settings.json`): only what the core doesn't
//! keep itself. Launch at login lives with the OS (the autostart plugin asks it), not here.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const FILE: &str = "settings.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// The game folder (or one product folder) the person chose, in place of the OS defaults.
    #[serde(default)]
    pub install_override: Option<PathBuf>,
}

impl Settings {
    /// The settings in `dir`, or the defaults when there are none or they can't be read.
    pub fn load(dir: &Path) -> Settings {
        std::fs::read(dir.join(FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Written whole through a temp file and a rename.
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join(format!(".{FILE}.tmp"));
        let body = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        let result =
            std::fs::write(&tmp, body).and_then(|()| std::fs::rename(&tmp, dir.join(FILE)));
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_missing_or_corrupt_is_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Settings::load(dir.path()), Settings::default());
        let s = Settings {
            install_override: Some(PathBuf::from("/Games/World of Warcraft")),
        };
        s.save(dir.path()).unwrap();
        assert_eq!(Settings::load(dir.path()), s);
        std::fs::write(dir.path().join(FILE), "{ not json").unwrap();
        assert_eq!(Settings::load(dir.path()), Settings::default());
    }
}
