// Vinyl Standard Library
// A collection of friendly wrappers for common Rust operations
// designed for VBA developers learning Rust via Vinyl.
//
// Each module wraps a standard Rust library or crate, and is a *namespace* of
// functions — you call them as `FileSystem::read(path)`, never an instance.
// Reading the source of each module is encouraged — it is real idiomatic Rust
// and a great learning resource.
//
// Every fallible function returns `Result<T, String>`. Vinyl hides that box:
// a normal call propagates the error; `Handle err` intercepts it; `Raw F()`
// yields the `Result` as a value.

// `FileSystem` and `Shell` are std-only on native hosts. They do not compile
// to wasm (`std::fs` / `std::process`), so they are cfg'd out there — Vinyl
// then lets Json/Regex link on a Page without dragging the rest along.
#[cfg(not(target_arch = "wasm32"))]
pub mod filesystem;
#[cfg(not(target_arch = "wasm32"))]
pub mod shell;
#[cfg(feature = "datetime")]
pub mod datetime;
#[cfg(feature = "json")]
pub mod json;
#[cfg(feature = "regex")]
pub mod regex;
#[cfg(feature = "http")]
pub mod http;
#[cfg(feature = "dataframe")]
pub mod dataframe;
#[cfg(feature = "database")]
pub mod database;

#[cfg(not(target_arch = "wasm32"))]
pub use filesystem::FileSystem;
#[cfg(not(target_arch = "wasm32"))]
pub use shell::{Process, Shell};
#[cfg(feature = "datetime")]
pub use datetime::DateTime;
#[cfg(feature = "json")]
pub use json::Json;
#[cfg(feature = "regex")]
pub use regex::Regex;
#[cfg(feature = "http")]
pub use http::Http;
#[cfg(feature = "dataframe")]
pub use dataframe::DataFrame;
#[cfg(feature = "database")]
pub use database::Database;
