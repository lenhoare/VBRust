// VBR Standard Library
// A collection of friendly wrappers for common Rust operations
// designed for VBA developers learning Rust via VBR.
//
// Each module wraps a standard Rust library or crate, and is a *namespace* of
// functions — you call them as `FileSystem::read(path)`, never an instance.
// Reading the source of each module is encouraged — it is real idiomatic Rust
// and a great learning resource.
//
// Every fallible function returns `Result<T, String>`, which maps onto VBR's
// `As Result<T>`.

pub mod datetime;
pub mod filesystem;
pub mod json;
pub mod regex;

// HTTP is deferred (see Cargo.toml). Database is a V2 feature (needs async).

pub use datetime::DateTime;
pub use filesystem::FileSystem;
pub use json::Json;
pub use regex::Regex;
