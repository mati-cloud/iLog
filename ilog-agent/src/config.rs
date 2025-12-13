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
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_flush_interval")]
    pub flush_interval_secs: u64,
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

fn default_batch_size() -> usize {
    100
}

fn default_flush_interval() -> u64 {
    5
}

impl AgentConfig {
    pub fn load(path: &PathBuf) -> Result<Self, ConfigError> {
        let config = Config::builder()
            .set_default("agent.batch_size", 100)?
            .set_default("agent.flush_interval_secs", 5)?
            .add_source(File::from(path.clone()))
            .add_source(Environment::with_prefix("ILOG_AGENT").separator("_"))
            .build()?;

        config.try_deserialize()
    }
}
