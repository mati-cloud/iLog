use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ParserConfig {
    pub sources: Vec<SourceConfig>,

    /// How often glob patterns are re-expanded to pick up newly created files,
    /// in seconds.
    ///
    /// Expanding once at startup is only correct for static paths. Under
    /// `/var/log/pods` every pod scheduled after boot writes to a path that did
    /// not exist during expansion, so a one-shot glob goes progressively blind
    /// as pods churn -- silently, which is the dangerous part.
    ///
    /// 15s trades a bounded discovery lag for not stat-ing the tree constantly.
    /// The lag costs nothing in practice: a new file is tailed from position 0,
    /// so lines written before discovery are still read.
    #[serde(default = "default_discovery_interval")]
    pub discovery_interval_secs: u64,
}

fn default_discovery_interval() -> u64 {
    15
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SourceConfig {
    pub name: String,
    pub path: String,

    /// Glob patterns to drop from `path`'s expansion.
    ///
    /// A single glob cannot express "everything except X", which matters most
    /// for the agent's own output: tailing the log of the backend you ship to
    /// means every ingest produces a line, which is itself ingested. That loop
    /// is self-sustaining and saturates the pipeline.
    #[serde(default)]
    pub exclude_paths: Vec<String>,

    #[serde(flatten)]
    pub parser: ParserType,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "format", rename_all = "lowercase")]
pub enum ParserType {
    Json {
        #[serde(default)]
        fields: Vec<FieldMapping>,
    },
    Regex {
        pattern: String,
        #[serde(default)]
        fields: Vec<FieldMapping>,
    },
    /// Kubernetes container logs under `/var/log/pods`.
    ///
    /// Strips the CRI envelope (timestamp, stream, partial flag), reassembles
    /// lines the runtime split at 16KiB, then parses the payload with `inner`.
    /// See [`crate::parser::cri`].
    Cri {
        /// Parser for the unwrapped payload. Defaults to JSON, which is what
        /// most containerised software emits on stdout; a payload that is not
        /// JSON falls back to `message` rather than being dropped.
        #[serde(default)]
        inner: Option<Box<ParserType>>,
        #[serde(default)]
        fields: Vec<FieldMapping>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FieldMapping {
    pub name: String,
    pub from: String,
    #[serde(default = "default_field_type")]
    pub field_type: FieldType,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    String,
    Int,
    Float,
    Bool,
}

fn default_field_type() -> FieldType {
    FieldType::String
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The unit tests in [`crate::parser::cri`] construct parsers directly, so
    /// they never exercise deserialization. This covers the path the deployed
    /// ConfigMap actually takes.
    #[test]
    fn k8s_source_deserializes() {
        let yaml = r#"
discovery_interval_secs: 15
sources:
  - name: k8s-pods
    path: /var/log/pods/*/*/*.log
    format: cri
    exclude_paths:
      - /var/log/pods/ilog_*/**
"#;
        let cfg: ParserConfig = serde_yaml::from_str(yaml).expect("parses");

        assert_eq!(cfg.discovery_interval_secs, 15);
        let source = &cfg.sources[0];
        assert_eq!(source.exclude_paths, vec!["/var/log/pods/ilog_*/**"]);
        assert!(matches!(source.parser, ParserType::Cri { inner: None, .. }));
    }

    #[test]
    fn defaults_apply_when_omitted() {
        let yaml = r#"
sources:
  - name: pods
    path: /var/log/pods/*/*/*.log
    format: cri
"#;
        let cfg: ParserConfig = serde_yaml::from_str(yaml).expect("parses");
        assert_eq!(cfg.discovery_interval_secs, 15);
        assert!(cfg.sources[0].exclude_paths.is_empty());
    }

    /// A nested inner parser must carry its own `format` tag.
    #[test]
    fn cri_with_explicit_inner_parser() {
        let yaml = r#"
sources:
  - name: pods
    path: /var/log/pods/*/*/*.log
    format: cri
    inner:
      format: regex
      pattern: '^(?P<message>.*)$'
"#;
        let cfg: ParserConfig = serde_yaml::from_str(yaml).expect("parses");
        match &cfg.sources[0].parser {
            ParserType::Cri { inner: Some(i), .. } => {
                assert!(matches!(**i, ParserType::Regex { .. }));
            }
            other => panic!("expected cri with inner regex, got {other:?}"),
        }
    }

    /// Existing configs must keep working -- the GitLab example in the repo has
    /// no `discovery_interval_secs` and no `exclude_paths`.
    #[test]
    fn legacy_json_and_regex_sources_still_parse() {
        let yaml = r#"
sources:
  - name: gitlab-production
    path: /var/log/gitlab/gitlab-rails/production_json.log
    format: json
  - name: nginx
    path: /var/log/gitlab/nginx/gitlab_access.log
    format: regex
    pattern: '^(?P<ip>\S+)'
    fields:
      - name: http.client_ip
        from: ip
"#;
        let cfg: ParserConfig = serde_yaml::from_str(yaml).expect("parses");
        assert_eq!(cfg.sources.len(), 2);
        assert!(matches!(cfg.sources[0].parser, ParserType::Json { .. }));
    }
}
