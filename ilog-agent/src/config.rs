use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct AgentConfig {
    pub agent: AgentSettings,
    #[serde(default)]
    pub sources: Sources,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AgentSettings {
    pub server: String,
    pub token: String,
    #[serde(default = "default_protocol")]
    pub protocol: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Sources {
    #[cfg(feature = "file")]
    pub file: Option<FileSource>,
    #[cfg(feature = "journald")]
    pub journald: Option<JournaldSource>,
    #[cfg(feature = "docker")]
    pub docker: Option<DockerSource>,
}

#[cfg(feature = "file")]
#[derive(Debug, Deserialize, Clone)]
pub struct FileSource {
    pub enabled: bool,
    pub paths: Vec<String>,
}

#[cfg(feature = "journald")]
#[derive(Debug, Deserialize, Clone)]
pub struct JournaldSource {
    pub enabled: bool,
    pub units: Vec<String>,
}

#[cfg(feature = "docker")]
#[derive(Debug, Deserialize, Clone)]
pub struct DockerSource {
    pub enabled: bool,
    pub containers: Vec<String>,
}

fn default_protocol() -> String {
    "tcp".to_string()
}

impl AgentConfig {
    pub fn load(path: &PathBuf) -> Result<Self, config::ConfigError> {
        let config = config::Config::builder()
            .set_default("agent.protocol", "tcp")?
            .add_source(config::File::from(path.clone()))
            .add_source(config::Environment::with_prefix("ILOG_AGENT").separator("_"))
            .build()?;

        config.try_deserialize()
    }
}
