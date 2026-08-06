pub mod config;
pub mod cri;
pub mod json;
pub mod regex_parser;

pub use config::{ParserConfig, ParserType, FieldMapping, SourceConfig};
pub use cri::CriParser;
pub use json::JsonParser;
pub use regex_parser::RegexParser;

use serde_json::Value;
use std::collections::HashMap;

pub trait LogParser: Send + Sync {
    fn parse(&self, line: &str) -> Option<HashMap<String, Value>>;
}

/// Build a parser from its config.
///
/// Returns a fresh instance per call rather than something shareable, because
/// [`CriParser`] carries partial-line reassembly state that must not be shared
/// across files. Callers tailing a glob expansion build one parser per path.
///
/// An invalid regex is a config error, not a runtime one -- it fails here so the
/// caller can skip that source with a clear message instead of silently matching
/// nothing forever.
pub fn build_parser(parser: &ParserType) -> Result<Box<dyn LogParser>, regex::Error> {
    match parser {
        ParserType::Json { fields } => Ok(Box::new(JsonParser::new(fields.clone()))),
        ParserType::Regex { pattern, fields } => {
            Ok(Box::new(RegexParser::new(pattern, fields.clone())?))
        }
        ParserType::Cri { inner, fields } => {
            // Absent `inner` means JSON: the overwhelmingly common shape for
            // container stdout, and harmless otherwise since a non-JSON payload
            // degrades to `message`.
            let inner = match inner {
                Some(cfg) => build_parser(cfg)?,
                None => Box::new(JsonParser::new(vec![])),
            };
            Ok(Box::new(CriParser::new(Some(inner), fields.clone())))
        }
    }
}
