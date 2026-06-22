//! Wraps `std::fs`, `std::io` and `std::path`.
//! The equivalent of VBA's `Scripting.FileSystemObject`, but native speed with
//! no COM overhead.

use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

pub struct FileSystem;

impl FileSystem {
    /// Read an entire file to a String.
    /// VBA equivalent: TextStream.ReadAll
    pub fn read(path: &str) -> Result<String, String> {
        fs::read_to_string(path).map_err(|e| e.to_string())
    }

    /// Read a file as a Vec of lines.
    /// VBA equivalent: reading line by line with TextStream
    pub fn read_lines(path: &str) -> Result<Vec<String>, String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        BufReader::new(file)
            .lines()
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| e.to_string())
    }

    /// Write a String to a file, creating or overwriting it.
    /// VBA equivalent: TextStream.Write after CreateTextFile
    pub fn write(path: &str, contents: &str) -> Result<(), String> {
        fs::write(path, contents).map_err(|e| e.to_string())
    }

    /// Append text to an existing file (creating it if needed).
    /// VBA equivalent: OpenTextFile with ForAppending
    pub fn append(path: &str, text: &str) -> Result<(), String> {
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())
    }

    /// Does a file exist?
    /// VBA equivalent: FSO.FileExists
    pub fn exists(path: &str) -> bool {
        Path::new(path).is_file()
    }

    /// Copy a file.
    /// VBA equivalent: FSO.CopyFile
    pub fn copy(source: &str, destination: &str) -> Result<(), String> {
        fs::copy(source, destination)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Move (rename) a file.
    /// VBA equivalent: FSO.MoveFile
    pub fn move_file(source: &str, destination: &str) -> Result<(), String> {
        fs::rename(source, destination).map_err(|e| e.to_string())
    }

    /// Delete a file.
    /// VBA equivalent: FSO.DeleteFile
    pub fn delete(path: &str) -> Result<(), String> {
        fs::remove_file(path).map_err(|e| e.to_string())
    }

    /// Create a folder.
    /// VBA equivalent: FSO.CreateFolder
    pub fn create_folder(path: &str) -> Result<(), String> {
        fs::create_dir(path).map_err(|e| e.to_string())
    }

    /// Create a folder and all its parent folders.
    pub fn create_folder_all(path: &str) -> Result<(), String> {
        fs::create_dir_all(path).map_err(|e| e.to_string())
    }

    /// Does a folder exist?
    /// VBA equivalent: FSO.FolderExists
    pub fn folder_exists(path: &str) -> bool {
        Path::new(path).is_dir()
    }

    /// Delete an empty folder.
    /// VBA equivalent: FSO.DeleteFolder
    pub fn delete_folder(path: &str) -> Result<(), String> {
        fs::remove_dir(path).map_err(|e| e.to_string())
    }

    /// Delete a folder and everything in it.
    pub fn delete_folder_all(path: &str) -> Result<(), String> {
        fs::remove_dir_all(path).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_and_read() {
        FileSystem::write("test_file.txt", "hello world").unwrap();
        assert_eq!(FileSystem::read("test_file.txt").unwrap(), "hello world");
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
        assert_eq!(FileSystem::read_lines("test_append.txt").unwrap().len(), 2);
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
