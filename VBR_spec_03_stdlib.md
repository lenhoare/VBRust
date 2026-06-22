## VBR Standard Library — Specification for Code Generation

---

IMPORTANT NOTE:
At first I agreed that VBR should follow Rust and use :: rather than . when referring to external libraries.
But I have changed my mind and would like VBR to just use .
Please ask to clarify when we come to implement.

I guess we create with Set fs= New FileSystem
We should use the Rust names like FileSystem excpet maybe 

## Project Structure

```
vbr_stdlib/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── filesystem.rs
    ├── json.rs
    ├── http.rs
    ├── database.rs
    └── datetime.rs
    ├── regex.rs
```

---

## Cargo.toml

```toml
[package]
name = "vbr_stdlib"
version = "0.1.0"
edition = "2021"
description = "VBR Standard Library — friendly wrappers for common Rust operations"
license = "MIT"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
reqwest = { version = "0.11", features = ["blocking"] }
chrono = "0.4"
regex = "1.0"

[dev-dependencies]
tokio = { version = "1", features = ["full"] }
```

---

## lib.rs

```rust
// VBR Standard Library
// A collection of friendly wrappers for common Rust operations
// designed for VBA developers learning Rust via VBR.
//
// Each module wraps a standard Rust library or crate.
// Reading the source of each module is encouraged —
// it is real idiomatic Rust and a great learning resource.

pub mod filesystem;
pub mod json;
pub mod http;
pub mod datetime;
pub mod regex;

// V2 — requires async support
// pub mod database;

pub use filesystem::FileSystem;
pub use json::Json;
pub use http::Http;
pub use datetime::DateTime;
pub use regex::Regex;
```

---

## filesystem.rs

### Purpose

Wraps `std::fs`, `std::io` and `std::path`. Equivalent of VBA's `Scripting.FileSystemObject` but native speed with no COM overhead.

### Implementation

```rust
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

pub struct FileSystem;

impl FileSystem {

    /// Read entire file contents to a String
    /// VBA equivalent: TextStream.ReadAll
    pub fn read(path: &str) -> Result<String, String> {
        fs::read_to_string(path)
            .map_err(|e| e.to_string())
    }

    /// Read file as a Vec of lines
    /// VBA equivalent: reading line by line with TextStream
    pub fn read_lines(path: &str) -> Result<Vec<String>, String> {
        let file = File::open(path)
            .map_err(|e| e.to_string())?;
        BufReader::new(file)
            .lines()
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| e.to_string())
    }

    /// Write a String to a file, creating or overwriting
    /// VBA equivalent: TextStream.Write after CreateTextFile
    pub fn write(path: &str, contents: &str) -> Result<(), String> {
        fs::write(path, contents)
            .map_err(|e| e.to_string())
    }

    /// Append text to an existing file
    /// VBA equivalent: OpenTextFile with ForAppending
    pub fn append(path: &str, text: &str) -> Result<(), String> {
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        file.write_all(text.as_bytes())
            .map_err(|e| e.to_string())
    }

    /// Check if a file exists
    /// VBA equivalent: FSO.FileExists
    pub fn exists(path: &str) -> bool {
        Path::new(path).is_file()
    }

    /// Copy a file from source to destination
    /// VBA equivalent: FSO.CopyFile
    pub fn copy(source: &str, destination: &str) -> Result<(), String> {
        fs::copy(source, destination)
            .map_err(|e| e.to_string())
            .map(|_| ())
    }

    /// Move a file from source to destination
    /// VBA equivalent: FSO.MoveFile
    pub fn move_file(source: &str, destination: &str) -> Result<(), String> {
        fs::rename(source, destination)
            .map_err(|e| e.to_string())
    }

    /// Delete a file
    /// VBA equivalent: FSO.DeleteFile
    pub fn delete(path: &str) -> Result<(), String> {
        fs::remove_file(path)
            .map_err(|e| e.to_string())
    }

    /// Create a folder
    /// VBA equivalent: FSO.CreateFolder
    pub fn create_folder(path: &str) -> Result<(), String> {
        fs::create_dir(path)
            .map_err(|e| e.to_string())
    }

    /// Create a folder and all parent folders
    /// VBA equivalent: FSO.CreateFolder with manual parent creation
    pub fn create_folder_all(path: &str) -> Result<(), String> {
        fs::create_dir_all(path)
            .map_err(|e| e.to_string())
    }

    /// Check if a folder exists
    /// VBA equivalent: FSO.FolderExists
    pub fn folder_exists(path: &str) -> bool {
        Path::new(path).is_dir()
    }

    /// Delete a folder
    /// VBA equivalent: FSO.DeleteFolder
    pub fn delete_folder(path: &str) -> Result<(), String> {
        fs::remove_dir(path)
            .map_err(|e| e.to_string())
    }

    /// Delete a folder and all its contents
    /// VBA equivalent: FSO.DeleteFolder
    pub fn delete_folder_all(path: &str) -> Result<(), String> {
        fs::remove_dir_all(path)
            .map_err(|e| e.to_string())
    }
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_and_read() {
        FileSystem::write("test_file.txt", "hello world").unwrap();
        let contents = FileSystem::read("test_file.txt").unwrap();
        assert_eq!(contents, "hello world");
        FileSystem::delete("test_file.txt").unwrap();
    }

    #[test]
    fn test_exists() {
        FileSystem::write("test_exists.txt", "test").unwrap();
        assert!(FileSystem::exists("test_exists.txt"));
        FileSystem::delete("test_exists.txt").unwrap();
        assert!(!FileSystem::exists("test_exists.txt"));
    }

    #[test]
    fn test_append() {
        FileSystem::write("test_append.txt", "line1\n").unwrap();
        FileSystem::append("test_append.txt", "line2\n").unwrap();
        let lines = FileSystem::read_lines("test_append.txt").unwrap();
        assert_eq!(lines.len(), 2);
        FileSystem::delete("test_append.txt").unwrap();
    }

    #[test]
    fn test_folder_operations() {
        FileSystem::create_folder("test_folder").unwrap();
        assert!(FileSystem::folder_exists("test_folder"));
        FileSystem::delete_folder("test_folder").unwrap();
        assert!(!FileSystem::folder_exists("test_folder"));
    }
}
```

---

## json.rs

### Purpose

Wraps `serde_json`. Equivalent of VBA's MSXML2 JSON parsing but clean and native.

### Implementation

```rust
use serde_json::{json, Value};

pub struct Json;

impl Json {

    /// Parse a JSON string into a Value
    /// VBA equivalent: parsing with MSXML2 or custom parser
    pub fn parse(text: &str) -> Result<Value, String> {
        serde_json::from_str(text)
            .map_err(|e| e.to_string())
    }

    /// Create an empty JSON object
    /// VBA equivalent: CreateObject("Scripting.Dictionary")
    pub fn object() -> Value {
        json!({})
    }

    /// Create an empty JSON array
    pub fn array() -> Value {
        json!([])
    }

    /// Serialise a Value to a JSON string
    pub fn to_string(value: &Value) -> Result<String, String> {
        serde_json::to_string(value)
            .map_err(|e| e.to_string())
    }

    /// Serialise a Value to a pretty printed JSON string
    pub fn to_pretty(value: &Value) -> Result<String, String> {
        serde_json::to_string_pretty(value)
            .map_err(|e| e.to_string())
    }

    /// Check if a key exists in a JSON object
    pub fn has_key(value: &Value, key: &str) -> bool {
        value.get(key).is_some()
    }

    /// Get a string value from a JSON object
    pub fn get_string(value: &Value, key: &str) -> Result<String, String> {
        value.get(key)
            .ok_or_else(|| format!("Key '{}' not found", key))?
            .as_str()
            .ok_or_else(|| format!("Key '{}' is not a string", key))
            .map(|s| s.to_string())
    }

    /// Get an integer value from a JSON object
    pub fn get_int(value: &Value, key: &str) -> Result<i64, String> {
        value.get(key)
            .ok_or_else(|| format!("Key '{}' not found", key))?
            .as_i64()
            .ok_or_else(|| format!("Key '{}' is not an integer", key))
    }

    /// Get a float value from a JSON object
    pub fn get_float(value: &Value, key: &str) -> Result<f64, String> {
        value.get(key)
            .ok_or_else(|| format!("Key '{}' not found", key))?
            .as_f64()
            .ok_or_else(|| format!("Key '{}' is not a float", key))
    }

    /// Get a boolean value from a JSON object
    pub fn get_bool(value: &Value, key: &str) -> Result<bool, String> {
        value.get(key)
            .ok_or_else(|| format!("Key '{}' not found", key))?
            .as_bool()
            .ok_or_else(|| format!("Key '{}' is not a boolean", key))
    }

    /// Get an array from a JSON object
    pub fn get_array(value: &Value, key: &str) -> Result<Vec<Value>, String> {
        value.get(key)
            .ok_or_else(|| format!("Key '{}' not found", key))?
            .as_array()
            .ok_or_else(|| format!("Key '{}' is not an array", key))
            .map(|a| a.clone())
    }

    /// Set a value in a JSON object
    pub fn set(value: &mut Value, key: &str, val: Value) {
        value[key] = val;
    }
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_get() {
        let data = Json::parse(r#"{"name":"Alice","age":42}"#).unwrap();
        assert_eq!(Json::get_string(&data, "name").unwrap(), "Alice");
        assert_eq!(Json::get_int(&data, "age").unwrap(), 42);
    }

    #[test]
    fn test_object_and_serialise() {
        let mut obj = Json::object();
        Json::set(&mut obj, "name", serde_json::json!("Bob"));
        let text = Json::to_string(&obj).unwrap();
        assert!(text.contains("Bob"));
    }

    #[test]
    fn test_has_key() {
        let data = Json::parse(r#"{"name":"Alice"}"#).unwrap();
        assert!(Json::has_key(&data, "name"));
        assert!(!Json::has_key(&data, "age"));
    }

    #[test]
    fn test_get_array() {
        let data = Json::parse(r#"{"items":[1,2,3]}"#).unwrap();
        let arr = Json::get_array(&data, "items").unwrap();
        assert_eq!(arr.len(), 3);
    }
}
```

---

## http.rs

### Purpose

Wraps `reqwest::blocking`. Synchronous HTTP — equivalent of VBA's WinHTTP or MSXML2.XMLHTTP but clean and native. Async HTTP is a V2 feature.

### Implementation

```rust
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::collections::HashMap;

pub struct Http;
pub struct HttpResponse(reqwest::blocking::Response);

impl Http {

    /// Simple GET request returning body as String
    /// VBA equivalent: WinHTTP.WinHttpRequest GET
    pub fn get(url: &str) -> Result<String, String> {
        reqwest::blocking::get(url)
            .map_err(|e| e.to_string())?
            .text()
            .map_err(|e| e.to_string())
    }

    /// GET request with custom headers
    pub fn get_with_headers(
        url: &str,
        headers: HashMap<String, String>
    ) -> Result<String, String> {
        let client = Client::new();
        let mut header_map = HeaderMap::new();
        for (key, value) in &headers {
            header_map.insert(
                HeaderName::from_bytes(key.as_bytes())
                    .map_err(|e| e.to_string())?,
                HeaderValue::from_str(value)
                    .map_err(|e| e.to_string())?
            );
        }
        client.get(url)
            .headers(header_map)
            .send()
            .map_err(|e| e.to_string())?
            .text()
            .map_err(|e| e.to_string())
    }

    /// POST request with a string body
    /// VBA equivalent: WinHTTP.WinHttpRequest POST
    pub fn post(url: &str, body: &str) -> Result<String, String> {
        Client::new()
            .post(url)
            .body(body.to_string())
            .send()
            .map_err(|e| e.to_string())?
            .text()
            .map_err(|e| e.to_string())
    }

    /// POST request with a JSON body
    pub fn post_json(
        url: &str,
        body: &serde_json::Value
    ) -> Result<String, String> {
        Client::new()
            .post(url)
            .json(body)
            .send()
            .map_err(|e| e.to_string())?
            .text()
            .map_err(|e| e.to_string())
    }

    /// GET request returning full response object
    /// Use when you need status code or headers
    pub fn get_response(url: &str) -> Result<HttpResponse, String> {
        Client::new()
            .get(url)
            .send()
            .map_err(|e| e.to_string())
            .map(HttpResponse)
    }

    /// Create an empty headers HashMap
    pub fn headers() -> HashMap<String, String> {
        HashMap::new()
    }
}

impl HttpResponse {

    /// Get the HTTP status code
    pub fn status(&self) -> u16 {
        self.0.status().as_u16()
    }

    /// Get the response body as String
    pub fn text(self) -> Result<String, String> {
        self.0.text()
            .map_err(|e| e.to_string())
    }

    /// Get a response header value
    pub fn header(&self, key: &str) -> Result<String, String> {
        self.0.headers()
            .get(key)
            .ok_or_else(|| format!("Header '{}' not found", key))?
            .to_str()
            .map_err(|e| e.to_string())
            .map(|s| s.to_string())
    }
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get() {
        let response = Http::get("https://httpbin.org/get").unwrap();
        assert!(response.contains("url"));
    }

    #[test]
    fn test_post() {
        let response = Http::post(
            "https://httpbin.org/post",
            "test body"
        ).unwrap();
        assert!(response.contains("test body"));
    }

    #[test]
    fn test_get_response_status() {
        let response = Http::get_response("https://httpbin.org/get").unwrap();
        assert_eq!(response.status(), 200);
    }

    #[test]
    fn test_headers() {
        let mut headers = Http::headers();
        headers.insert("Authorization".to_string(), "Bearer test".to_string());
        let response = Http::get_with_headers(
            "https://httpbin.org/headers",
            headers
        ).unwrap();
        assert!(response.contains("Bearer test"));
    }
}
```

---

## datetime.rs

### Purpose

Wraps `chrono`. Replaces VBA's built in date functions with a proper date/time library.

### Implementation

```rust
use chrono::{DateTime as ChronoDateTime, Duration, Local, 
             NaiveDateTime, TimeZone, Utc};

pub struct DateTime;

impl DateTime {

    /// Get current local date and time
    /// VBA equivalent: Now()
    pub fn now() -> ChronoDateTime<Local> {
        Local::now()
    }

    /// Get current UTC date and time
    /// VBA equivalent: Now() but UTC
    pub fn utc() -> ChronoDateTime<Utc> {
        Utc::now()
    }

    /// Format a datetime as a string
    /// VBA equivalent: Format(date, "pattern")
    pub fn format(dt: &ChronoDateTime<Local>, pattern: &str) -> String {
        dt.format(pattern).to_string()
    }

    /// Parse a datetime from a string
    /// VBA equivalent: CDate()
    pub fn parse(text: &str, pattern: &str) 
        -> Result<NaiveDateTime, String> {
        NaiveDateTime::parse_from_str(text, pattern)
            .map_err(|e| e.to_string())
    }

    /// Add days to a datetime
    /// VBA equivalent: DateAdd("d", n, date)
    pub fn add_days(
        dt: ChronoDateTime<Local>, 
        days: i64
    ) -> ChronoDateTime<Local> {
        dt + Duration::days(days)
    }

    /// Add hours to a datetime
    /// VBA equivalent: DateAdd("h", n, date)
    pub fn add_hours(
        dt: ChronoDateTime<Local>, 
        hours: i64
    ) -> ChronoDateTime<Local> {
        dt + Duration::hours(hours)
    }

    /// Add minutes to a datetime
    /// VBA equivalent: DateAdd("n", n, date)
    pub fn add_minutes(
        dt: ChronoDateTime<Local>, 
        minutes: i64
    ) -> ChronoDateTime<Local> {
        dt + Duration::minutes(minutes)
    }

    /// Difference in days between two datetimes
    /// VBA equivalent: DateDiff("d", date1, date2)
    pub fn diff_days(
        dt1: ChronoDateTime<Local>, 
        dt2: ChronoDateTime<Local>
    ) -> i64 {
        (dt2 - dt1).num_days()
    }

    /// Difference in hours between two datetimes
    /// VBA equivalent: DateDiff("h", date1, date2)
    pub fn diff_hours(
        dt1: ChronoDateTime<Local>, 
        dt2: ChronoDateTime<Local>
    ) -> i64 {
        (dt2 - dt1).num_hours()
    }

    /// Get year from datetime
    /// VBA equivalent: Year(date)
    pub fn year(dt: &ChronoDateTime<Local>) -> i32 {
        dt.format("%Y").to_string().parse().unwrap()
    }

    /// Get month from datetime
    /// VBA equivalent: Month(date)
    pub fn month(dt: &ChronoDateTime<Local>) -> u32 {
        dt.format("%m").to_string().parse().unwrap()
    }

    /// Get day from datetime
    /// VBA equivalent: Day(date)
    pub fn day(dt: &ChronoDateTime<Local>) -> u32 {
        dt.format("%d").to_string().parse().unwrap()
    }
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_now_and_format() {
        let now = DateTime::now();
        let formatted = DateTime::format(&now, "%Y-%m-%d");
        assert_eq!(formatted.len(), 10);
    }

    #[test]
    fn test_add_days() {
        let now = DateTime::now();
        let tomorrow = DateTime::add_days(now, 1);
        assert_eq!(DateTime::diff_days(now, tomorrow), 1);
    }

    #[test]
    fn test_parse() {
        let dt = DateTime::parse(
            "2024-01-15 10:30:00", 
            "%Y-%m-%d %H:%M:%S"
        ).unwrap();
        assert_eq!(dt.to_string(), "2024-01-15 10:30:00");
    }
}
```

---

## regex.rs

### Purpose

Wraps the `regex` crate. Equivalent of VBA's `VBScript.RegExp` object.

### Implementation

```rust
use regex::Regex as RegexEngine;

pub struct Regex;

impl Regex {

    /// Check if a pattern matches anywhere in text
    /// VBA equivalent: RegExp.Test
    pub fn is_match(pattern: &str, text: &str) -> Result<bool, String> {
        RegexEngine::new(pattern)
            .map_err(|e| e.to_string())
            .map(|re| re.is_match(text))
    }

    /// Find first match and return it
    /// VBA equivalent: RegExp.Execute — first match
    pub fn find(pattern: &str, text: &str) 
        -> Result<Option<String>, String> {
        let re = RegexEngine::new(pattern)
            .map_err(|e| e.to_string())?;
        Ok(re.find(text).map(|m| m.as_str().to_string()))
    }

    /// Find all matches and return as Vec
    /// VBA equivalent: RegExp.Execute — all matches
    pub fn find_all(pattern: &str, text: &str) 
        -> Result<Vec<String>, String> {
        let re = RegexEngine::new(pattern)
            .map_err(|e| e.to_string())?;
        Ok(re.find_iter(text)
            .map(|m| m.as_str().to_string())
            .collect())
    }

    /// Replace first match
    /// VBA equivalent: RegExp.Replace with Global = False
    pub fn replace(
        pattern: &str, 
        text: &str, 
        replacement: &str
    ) -> Result<String, String> {
        RegexEngine::new(pattern)
            .map_err(|e| e.to_string())
            .map(|re| re.replace(text, replacement).to_string())
    }

    /// Replace all matches
    /// VBA equivalent: RegExp.Replace with Global = True
    pub fn replace_all(
        pattern: &str, 
        text: &str, 
        replacement: &str
    ) -> Result<String, String> {
        RegexEngine::new(pattern)
            .map_err(|e| e.to_string())
            .map(|re| re.replace_all(text, replacement).to_string())
    }

    /// Get capture groups from first match
    /// VBA equivalent: RegExp.Execute — SubMatches
    pub fn captures(
        pattern: &str, 
        text: &str
    ) -> Result<Vec<String>, String> {
        let re = RegexEngine::new(pattern)
            .map_err(|e| e.to_string())?;
        Ok(re.captures(text)
            .map(|caps| caps.iter()
                .skip(1)
                .filter_map(|m| m.map(|m| m.as_str().to_string()))
                .collect())
            .unwrap_or_default())
    }
}
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_match() {
        assert!(Regex::is_match(r"\d+", "hello 123").unwrap());
        assert!(!Regex::is_match(r"\d+", "hello world").unwrap());
    }

    #[test]
    fn test_find() {
        let result = Regex::find(r"\d+", "hello 123 world").unwrap();
        assert_eq!(result, Some("123".to_string()));
    }

    #[test]
    fn test_find_all() {
        let results = Regex::find_all(r"\d+", "1 and 2 and 3").unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_replace_all() {
        let result = Regex::replace_all(
            r"\d+", 
            "1 and 2 and 3", 
            "NUM"
        ).unwrap();
        assert_eq!(result, "NUM and NUM and NUM");
    }

    #[test]
    fn test_captures() {
        let caps = Regex::captures(
            r"(\w+)\s(\w+)", 
            "hello world"
        ).unwrap();
        assert_eq!(caps[0], "hello");
        assert_eq!(caps[1], "world");
    }
}
```

---

## Summary For Code Generation Agent

|File|Purpose|External Crate|
|---|---|---|
|`Cargo.toml`|Project definition and dependencies|—|
|`lib.rs`|Module declarations and public exports|—|
|`filesystem.rs`|File and folder operations|None — std only|
|`json.rs`|JSON parsing and generation|`serde_json`|
|`http.rs`|Synchronous HTTP requests|`reqwest` blocking|
|`datetime.rs`|Date and time operations|`chrono`|
|`regex.rs`|Regular expressions|`regex`|

Each file must:

- Include full implementation as specified
- Include all tests in `#[cfg(test)]` module
- Include VBA equivalent comments on every public function
- Follow Rust naming conventions throughout
- Return `Result<T, String>` for all fallible operations