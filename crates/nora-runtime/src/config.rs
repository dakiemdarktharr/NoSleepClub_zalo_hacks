use crate::Result;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    pub state_dir: PathBuf,
    pub llm_config: PathBuf,
    pub asset_root: PathBuf,
    #[serde(default)]
    pub quiet: bool,
    #[serde(default)]
    pub public_research: bool,
    #[serde(default = "default_recording_seconds")]
    pub max_recording_seconds: u32,
}

fn default_recording_seconds() -> u32 {
    30
}

impl AppConfig {
    pub fn load_preferences(&mut self) -> Result<()> {
        let path = self.state_dir.join("preferences.json");
        if !path.exists() {
            return Ok(());
        }
        let file = File::open(path)?;
        if file.metadata()?.len() > 4096 {
            return Err("preferences exceed bound".into());
        }
        let mut bytes = Vec::new();
        file.take(4097).read_to_end(&mut bytes)?;
        if bytes.len() > 4096 {
            return Err("preferences exceed bound".into());
        }
        let p: Preferences = serde_json::from_slice(&bytes)?;
        self.quiet = p.quiet;
        self.public_research = p.public_research;
        Ok(())
    }
    pub fn save_preferences(&self) -> Result<()> {
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new_in(&self.state_dir)?;
        serde_json::to_writer(
            &mut file,
            &Preferences {
                quiet: self.quiet,
                public_research: self.public_research,
            },
        )?;
        file.flush()?;
        file.as_file().sync_all()?;
        file.persist(self.state_dir.join("preferences.json"))?;
        Ok(())
    }
    pub fn defaults(executable: &Path) -> Result<Self> {
        let bin = executable
            .parent()
            .ok_or("executable has no parent directory")?;
        let install = if bin.file_name().is_some_and(|s| s == "bin") {
            bin.parent().ok_or("invalid installation root")?
        } else {
            bin
        };
        let local = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| install.join("state"));
        let local_config = std::env::current_dir()?.join(".env.local");
        Ok(Self {
            state_dir: local.join("NoSleepClub").join("NORA"),
            llm_config: if local_config.is_file() {
                local_config
            } else {
                install.join("config").join(".env.local")
            },
            asset_root: install.join("assets"),
            quiet: false,
            public_research: false,
            max_recording_seconds: 30,
        })
    }

    pub fn load(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        if !file.metadata()?.is_file() || file.metadata()?.len() > 65_536 {
            return Err("application configuration exceeds limit".into());
        }
        let mut bytes = Vec::new();
        file.take(65_537).read_to_end(&mut bytes)?;
        if bytes.len() > 65_536 {
            return Err("application configuration exceeds limit".into());
        }
        let mut config: Self = serde_json::from_slice(&bytes)?;
        if !(1..=60).contains(&config.max_recording_seconds) {
            return Err("recording duration must be in1..=60 seconds".into());
        }
        let base = path.parent().unwrap_or(Path::new("."));
        for target in [
            &mut config.state_dir,
            &mut config.llm_config,
            &mut config.asset_root,
        ] {
            if target.as_os_str().is_empty() {
                return Err("empty application path".into());
            }
            if target.is_relative() {
                *target = base.join(&*target);
            }
        }
        Ok(config)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Preferences {
    quiet: bool,
    public_research: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configuration_rejects_unknown_fields_and_bad_recording_bounds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.json");
        std::fs::write(&path,r#"{"state_dir":"s","llm_config":".env.local","asset_root":"a","max_recording_seconds":999}"#).unwrap();
        assert!(AppConfig::load(&path).is_err());
        std::fs::write(&path,r#"{"state_dir":"s","llm_config":".env.local","asset_root":"a","enable_arbitrary_shell":true}"#).unwrap();
        assert!(AppConfig::load(&path).is_err());
    }
}
