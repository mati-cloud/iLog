use super::{LogParser, FieldMapping, config::FieldType};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;

pub struct RegexParser {
    regex: Regex,
    fields: Vec<FieldMapping>,
}

impl RegexParser {
    pub fn new(pattern: &str, fields: Vec<FieldMapping>) -> Result<Self, regex::Error> {
        Ok(Self {
            regex: Regex::new(pattern)?,
            fields,
        })
    }
}

impl LogParser for RegexParser {
    fn parse(&self, line: &str) -> Option<HashMap<String, Value>> {
        let captures = self.regex.captures(line)?;
        let mut result = HashMap::new();
        
        if self.fields.is_empty() {
            // No field mapping - use named groups from regex
            for name in self.regex.capture_names().flatten() {
                if let Some(value) = captures.name(name) {
                    result.insert(name.to_string(), Value::String(value.as_str().to_string()));
                }
            }
        } else {
            // Apply field mappings
            for field in &self.fields {
                if let Some(value) = captures.name(&field.from) {
                    let str_value = value.as_str();
                    let converted = match field.field_type {
                        FieldType::String => Value::String(str_value.to_string()),
                        FieldType::Int => str_value.parse::<i64>().ok().map(Value::from).unwrap_or(Value::String(str_value.to_string())),
                        FieldType::Float => str_value.parse::<f64>().ok().map(Value::from).unwrap_or(Value::String(str_value.to_string())),
                        FieldType::Bool => str_value.parse::<bool>().ok().map(Value::from).unwrap_or(Value::String(str_value.to_string())),
                    };
                    result.insert(field.name.clone(), converted);
                }
            }
        }
        
        Some(result)
    }
}
