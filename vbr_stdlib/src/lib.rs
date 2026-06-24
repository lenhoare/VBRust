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

// `FileSystem` is std-only and always available; the rest are behind features
// (see Cargo.toml) so a project compiles only the wrappers it actually uses.
pub mod filesystem;
#[cfg(feature = "datetime")]
pub mod datetime;
#[cfg(feature = "json")]
pub mod json;
#[cfg(feature = "regex")]
pub mod regex;
#[cfg(feature = "http")]
pub mod http;

pub use filesystem::FileSystem;
#[cfg(feature = "datetime")]
pub use datetime::DateTime;
#[cfg(feature = "json")]
pub use json::Json;
#[cfg(feature = "regex")]
pub use regex::Regex;
#[cfg(feature = "http")]
pub use http::Http;
