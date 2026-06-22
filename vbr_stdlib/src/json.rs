//! Wraps `serde_json`. A `Json` value is a parsed (or built) JSON document —
//! construct one with `Json::parse(...)`, `Json::object()` or `Json::array()`,
//! then read fields with the `get_*` methods.

use serde_json::{json, Value};

#[derive(Clone)]
pub struct Json(Value);

impl Json {
    /// Parse a JSON string.
    pub fn parse(text: &str) -> Result<Json, String> {
        serde_json::from_str(text).map(Json).map_err(|e| e.to_string())
    }

    /// A new empty JSON object.
    /// VBA equivalent: CreateObject("Scripting.Dictionary")
    pub fn object() -> Json {
        Json(json!({}))
    }

    /// A new empty JSON array.
    pub fn array() -> Json {
        Json(json!([]))
    }

    /// Serialise to a compact JSON string.
    pub fn to_string(&self) -> Result<String, String> {
        serde_json::to_string(&self.0).map_err(|e| e.to_string())
    }

    /// Serialise to a pretty-printed JSON string.
    pub fn to_pretty(&self) -> Result<String, String> {
        serde_json::to_string_pretty(&self.0).map_err(|e| e.to_string())
    }

    /// Does this object have `key`?
    pub fn has_key(&self, key: &str) -> bool {
        self.0.get(key).is_some()
    }

    /// Get a string field.
    pub fn get_string(&self, key: &str) -> Result<String, String> {
        self.field(key)?
            .as_str()
            .ok_or_else(|| format!("Key '{}' is not a string", key))
            .map(|s| s.to_string())
    }

    /// Get an integer field.
    pub fn get_int(&self, key: &str) -> Result<i64, String> {
        self.field(key)?
            .as_i64()
            .ok_or_else(|| format!("Key '{}' is not an integer", key))
    }

    /// Get a float field.
    pub fn get_float(&self, key: &str) -> Result<f64, String> {
        self.field(key)?
            .as_f64()
            .ok_or_else(|| format!("Key '{}' is not a float", key))
    }

    /// Get a boolean field.
    pub fn get_bool(&self, key: &str) -> Result<bool, String> {
        self.field(key)?
            .as_bool()
            .ok_or_else(|| format!("Key '{}' is not a boolean", key))
    }

    /// Get an array field, as a Vec of Json values.
    pub fn get_array(&self, key: &str) -> Result<Vec<Json>, String> {
        self.field(key)?
            .as_array()
            .ok_or_else(|| format!("Key '{}' is not an array", key))
            .map(|a| a.iter().cloned().map(Json).collect())
    }

    /// Get a nested object/value field.
    pub fn get(&self, key: &str) -> Result<Json, String> {
        self.field(key).map(|v| Json(v.clone()))
    }

    /// Set a string field.
    pub fn set_string(&mut self, key: &str, val: &str) {
        self.0[key] = json!(val);
    }

    /// Set an integer field.
    pub fn set_int(&mut self, key: &str, val: i64) {
        self.0[key] = json!(val);
    }

    /// Set a boolean field.
    pub fn set_bool(&mut self, key: &str, val: bool) {
        self.0[key] = json!(val);
    }

    /// Set a field to another Json value.
    pub fn set(&mut self, key: &str, val: &Json) {
        self.0[key] = val.0.clone();
    }

    /// Read this value itself as a string (for array elements / scalars).
    pub fn as_string(&self) -> Result<String, String> {
        self.0
            .as_str()
            .ok_or_else(|| "value is not a string".to_string())
            .map(|s| s.to_string())
    }

    /// Read this value itself as an integer.
    pub fn as_int(&self) -> Result<i64, String> {
        self.0.as_i64().ok_or_else(|| "value is not an integer".to_string())
    }

    /// Read this value itself as a float.
    pub fn as_float(&self) -> Result<f64, String> {
        self.0.as_f64().ok_or_else(|| "value is not a float".to_string())
    }

    /// Read this value itself as a boolean.
    pub fn as_bool(&self) -> Result<bool, String> {
        self.0.as_bool().ok_or_else(|| "value is not a boolean".to_string())
    }

    fn field(&self, key: &str) -> Result<&Value, String> {
        self.0.get(key).ok_or_else(|| format!("Key '{}' not found", key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_get() {
        let data = Json::parse(r#"{"name":"Alice","age":42,"member":true}"#).unwrap();
        assert_eq!(data.get_string("name").unwrap(), "Alice");
        assert_eq!(data.get_int("age").unwrap(), 42);
        assert!(data.get_bool("member").unwrap());
    }

    #[test]
    fn test_object_and_serialise() {
        let mut obj = Json::object();
        obj.set_string("name", "Bob");
        obj.set_int("age", 30);
        assert!(obj.to_string().unwrap().contains("Bob"));
        assert_eq!(obj.get_int("age").unwrap(), 30);
    }

    #[test]
    fn test_has_key_and_array() {
        let data = Json::parse(r#"{"items":[1,2,3]}"#).unwrap();
        assert!(data.has_key("items"));
        assert!(!data.has_key("missing"));
        assert_eq!(data.get_array("items").unwrap().len(), 3);
    }
}
