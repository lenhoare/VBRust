//! Wraps the `regex` crate. The equivalent of VBA's `VBScript.RegExp` object.

use regex::Regex as RegexEngine;

pub struct Regex;

impl Regex {
    /// Does the pattern match anywhere in the text?
    /// VBA equivalent: RegExp.Test
    pub fn is_match(pattern: &str, text: &str) -> Result<bool, String> {
        RegexEngine::new(pattern)
            .map_err(|e| e.to_string())
            .map(|re| re.is_match(text))
    }

    /// Find the first match.
    /// VBA equivalent: RegExp.Execute — first match
    pub fn find(pattern: &str, text: &str) -> Result<Option<String>, String> {
        let re = RegexEngine::new(pattern).map_err(|e| e.to_string())?;
        Ok(re.find(text).map(|m| m.as_str().to_string()))
    }

    /// Find all matches.
    /// VBA equivalent: RegExp.Execute — all matches
    pub fn find_all(pattern: &str, text: &str) -> Result<Vec<String>, String> {
        let re = RegexEngine::new(pattern).map_err(|e| e.to_string())?;
        Ok(re.find_iter(text).map(|m| m.as_str().to_string()).collect())
    }

    /// Replace the first match.
    /// VBA equivalent: RegExp.Replace with Global = False
    pub fn replace(pattern: &str, text: &str, replacement: &str) -> Result<String, String> {
        RegexEngine::new(pattern)
            .map_err(|e| e.to_string())
            .map(|re| re.replace(text, replacement).to_string())
    }

    /// Replace all matches.
    /// VBA equivalent: RegExp.Replace with Global = True
    pub fn replace_all(pattern: &str, text: &str, replacement: &str) -> Result<String, String> {
        RegexEngine::new(pattern)
            .map_err(|e| e.to_string())
            .map(|re| re.replace_all(text, replacement).to_string())
    }

    /// Capture groups from the first match (group 1 onward).
    /// VBA equivalent: RegExp.Execute — SubMatches
    pub fn captures(pattern: &str, text: &str) -> Result<Vec<String>, String> {
        let re = RegexEngine::new(pattern).map_err(|e| e.to_string())?;
        Ok(re
            .captures(text)
            .map(|caps| {
                caps.iter()
                    .skip(1)
                    .filter_map(|m| m.map(|m| m.as_str().to_string()))
                    .collect()
            })
            .unwrap_or_default())
    }
}

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
        assert_eq!(
            Regex::find(r"\d+", "hello 123 world").unwrap(),
            Some("123".to_string())
        );
    }

    #[test]
    fn test_find_all() {
        assert_eq!(Regex::find_all(r"\d+", "1 and 2 and 3").unwrap().len(), 3);
    }

    #[test]
    fn test_replace_all() {
        assert_eq!(
            Regex::replace_all(r"\d+", "1 and 2 and 3", "NUM").unwrap(),
            "NUM and NUM and NUM"
        );
    }

    #[test]
    fn test_captures() {
        let caps = Regex::captures(r"(\w+)\s(\w+)", "hello world").unwrap();
        assert_eq!(caps[0], "hello");
        assert_eq!(caps[1], "world");
    }
}
