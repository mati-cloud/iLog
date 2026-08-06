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
            // Prefix is ILOG, not ILOG_AGENT: the prefix is stripped and the
            // remainder is read as the config path, so ILOG_AGENT would have
            // required ILOG_AGENT_AGENT_TOKEN to reach agent.token. With ILOG the
            // env names match the config structure directly --
            // ILOG_AGENT_TOKEN -> agent.token, ILOG_AGENT_SERVER -> agent.server.
            .add_source(config::Environment::with_prefix("ILOG").separator("_"))
            .build()?;

        config.try_deserialize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Env overrides are how the Kubernetes DaemonSet injects the token, so the
    /// prefix getting out of step with the config structure is a deploy-time
    /// crashloop rather than a compile error. This pins the names the manifest
    /// actually sets.
    ///
    /// Serialised because it mutates process env.
    #[test]
    fn env_overrides_map_to_agent_fields() {
        let dir = std::env::temp_dir().join(format!("ilog-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");

        // No token in the file: the DaemonSet supplies it from a Secret, so this
        // deserialises only if the env override is picked up.
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "[agent]\nserver = \"unused:1\"\nprotocol = \"tcp\"").unwrap();

        std::env::set_var("ILOG_AGENT_TOKEN", "agt_abc_def");
        std::env::set_var("ILOG_AGENT_SERVER", "backend.ilog.svc.cluster.local:8081");

        let cfg = AgentConfig::load(&path).expect("env overrides supply the token");

        assert_eq!(cfg.agent.token, "agt_abc_def");
        assert_eq!(cfg.agent.server, "backend.ilog.svc.cluster.local:8081");

        std::env::remove_var("ILOG_AGENT_TOKEN");
        std::env::remove_var("ILOG_AGENT_SERVER");
        std::fs::remove_dir_all(&dir).ok();
    }
}
