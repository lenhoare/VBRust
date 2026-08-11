//! `tide-editor` — reusable TUI code editor for ratatui.
//!
//! Document / view / decoration model inspired by Monaco: hosts own the
//! language and IDE chrome; this crate owns editing and painting.

mod buffer;
mod clipboard;
mod input;
mod language;
mod style;
mod view;
mod widget;

pub use buffer::Document;
pub use clipboard::{copy_to_clipboard, paste_from_clipboard};
pub use input::{Key, KeyEvent, KeyMods};
pub use language::{Highlighter, KeywordHighlighter, PlainHighlighter};
pub use style::{palette, Decoration, SpanStyle};
pub use view::{is_ctrl, EditorView, Selection};
pub use widget::EditorWidget;
