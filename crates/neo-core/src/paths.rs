use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::Platform;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppPaths {
    pub settings_path: PathBuf,
    pub log_directory: PathBuf,
    pub gstreamer_registry_path: Option<PathBuf>,
    pub engine_directory: PathBuf,
    pub engine_package_cache_directory: PathBuf,
    pub e2e_profile: bool,
}

#[derive(Debug, Error)]
pub enum PathsError {
    #[error("required user directory is unavailable: {0}")]
    MissingDirectory(&'static str),
}

impl AppPaths {
    pub fn new(e2e_profile: Option<&Path>) -> Result<Self, PathsError> {
        if let Some(profile) = e2e_profile {
            return Self::for_e2e_profile(profile);
        }

        let config_root = dirs::config_dir().ok_or(PathsError::MissingDirectory("config"))?;
        let data_root = dirs::data_dir().ok_or(PathsError::MissingDirectory("data"))?;
        let cache_root = dirs::cache_dir().ok_or(PathsError::MissingDirectory("cache"))?;

        let settings_directory = config_root.join("NeoAirPlay");
        let (log_directory, gstreamer_registry_path, engine_root, package_cache_root) =
            match Platform::current() {
                Platform::Windows => {
                    let local_root = dirs::data_local_dir()
                        .ok_or(PathsError::MissingDirectory("local data"))?
                        .join("NeoAirPlay");
                    (
                        local_root.join("Logs"),
                        Some(local_root.join("gstreamer-registry.bin")),
                        local_root.clone(),
                        local_root,
                    )
                }
                Platform::Macos => {
                    let home = dirs::home_dir().ok_or(PathsError::MissingDirectory("home"))?;
                    (
                        home.join("Library").join("Logs").join("NeoAirPlay"),
                        None,
                        data_root.join("NeoAirPlay"),
                        cache_root.join("NeoAirPlay"),
                    )
                }
                Platform::Linux => (
                    xdg_state_dir()?.join("NeoAirPlay").join("logs"),
                    None,
                    data_root.join("NeoAirPlay"),
                    cache_root.join("NeoAirPlay"),
                ),
            };

        Ok(Self {
            settings_path: settings_directory.join("settings.json"),
            log_directory,
            gstreamer_registry_path,
            engine_directory: engine_root.join("engine"),
            engine_package_cache_directory: package_cache_root.join("EnginePackages"),
            e2e_profile: false,
        })
    }

    fn for_e2e_profile(profile: &Path) -> Result<Self, PathsError> {
        let profile = if profile.is_absolute() {
            profile.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|_| PathsError::MissingDirectory("current directory"))?
                .join(profile)
        };
        let roaming_root = profile.join("roaming").join("NeoAirPlay");
        let local_root = profile.join("local").join("NeoAirPlay");

        Ok(Self {
            settings_path: roaming_root.join("settings.json"),
            log_directory: local_root.join("Logs"),
            gstreamer_registry_path: Some(local_root.join("gstreamer-registry.bin")),
            engine_directory: local_root.join("engine"),
            engine_package_cache_directory: local_root.join("EnginePackages"),
            e2e_profile: true,
        })
    }
}

fn xdg_state_dir() -> Result<PathBuf, PathsError> {
    if let Some(path) = std::env::var_os("XDG_STATE_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    dirs::home_dir()
        .map(|home| home.join(".local").join("state"))
        .ok_or(PathsError::MissingDirectory("state"))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::AppPaths;

    #[test]
    fn e2e_profile_redirects_all_persistent_paths() {
        let paths = AppPaths::new(Some(Path::new("/tmp/neo-airplay-e2e")))
            .expect("profile paths should be created");

        assert!(paths.e2e_profile);
        assert_eq!(
            paths.settings_path,
            Path::new("/tmp/neo-airplay-e2e/roaming/NeoAirPlay/settings.json")
        );
        assert_eq!(
            paths.log_directory,
            Path::new("/tmp/neo-airplay-e2e/local/NeoAirPlay/Logs")
        );
        assert_eq!(
            paths.engine_directory,
            Path::new("/tmp/neo-airplay-e2e/local/NeoAirPlay/engine")
        );
        assert_eq!(
            paths.engine_package_cache_directory,
            Path::new("/tmp/neo-airplay-e2e/local/NeoAirPlay/EnginePackages")
        );
    }
}
