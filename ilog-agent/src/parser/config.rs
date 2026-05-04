use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ParserConfig {
    pub sources: Vec<SourceConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SourceConfig {
    pub name: String,
    pub path: String,
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
