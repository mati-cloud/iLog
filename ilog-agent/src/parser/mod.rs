pub mod config;
pub mod json;
pub mod regex_parser;

pub use config::{ParserConfig, ParserType, FieldMapping, SourceConfig};
pub use json::JsonParser;
pub use regex_parser::RegexParser;

use serde_json::Value;
use std::collections::HashMap;

pub trait LogParser: Send + Sync {
    fn parse(&self, line: &str) -> Option<HashMap<String, Value>>;
}
