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

    /// Names in a folder. Directories end with `/`; hidden (dot) names are skipped.
    /// Folders come first, then files, both sorted case-insensitively.
    pub fn list(path: &str) -> Result<Vec<String>, String> {
        let mut dirs = Vec::new();
        let mut files = Vec::new();
        for ent in fs::read_dir(path).map_err(|e| e.to_string())? {
            let ent = ent.map_err(|e| e.to_string())?;
            let name = ent.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            if ent.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                dirs.push(format!("{name}/"));
            } else {
                files.push(name);
            }
        }
        let fold = |a: &String, b: &String| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase());
        dirs.sort_by(fold);
        files.sort_by(fold);
        dirs.append(&mut files);
        Ok(dirs)
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

    #[test]
    fn test_list_dirs_then_files() {
        FileSystem::create_folder_all("_vbr_list_test/sub").unwrap();
        FileSystem::write("_vbr_list_test/b.txt", "b").unwrap();
        FileSystem::write("_vbr_list_test/a.txt", "a").unwrap();
        FileSystem::write("_vbr_list_test/.hidden", "no").unwrap();
        let names = FileSystem::list("_vbr_list_test").unwrap();
        FileSystem::delete_folder_all("_vbr_list_test").unwrap();
        assert_eq!(names, vec!["sub/".to_string(), "a.txt".to_string(), "b.txt".to_string()]);
    }
}
