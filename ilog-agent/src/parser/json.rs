use super::{LogParser, FieldMapping};
use serde_json::Value;
use std::collections::HashMap;

pub struct JsonParser {
    fields: Vec<FieldMapping>,
}

impl JsonParser {
    pub fn new(fields: Vec<FieldMapping>) -> Self {
        Self { fields }
    }
}

impl LogParser for JsonParser {
    fn parse(&self, line: &str) -> Option<HashMap<String, Value>> {
        let parsed: Value = serde_json::from_str(line).ok()?;
        
        if self.fields.is_empty() {
            // No field mapping - return all fields
            if let Value::Object(map) = parsed {
                return Some(map.into_iter().collect());
            }
            return None;
        }
        
        // Apply field mappings
        let mut result = HashMap::new();
        for field in &self.fields {
            if let Some(value) = parsed.get(&field.from) {
                result.insert(field.name.clone(), value.clone());
            }
        }
        
        Some(result)
    }
}
