//! Internal clipboard + quiet terminal clipboard (OSC 52).
//!
//! Avoids OS clipboard crates (`arboard`, `xclip`, …) which often print to
//! stderr or fight the host clipboard under WSL and corrupt a ratatui screen.

use std::io::{self, Write};

/// Copy into the in-process clipboard and, when the terminal supports it, the
/// host clipboard via OSC 52 (no subprocess, no stderr).
pub fn copy_to_clipboard(internal: &mut String, text: &str) {
    *internal = text.to_string();
    let _ = write_osc52(text);
}

/// Paste from the in-process clipboard (Ctrl+V). Prefer terminal bracketed
/// paste (`Event::Paste`) for content coming from outside the app.
pub fn paste_from_clipboard(internal: &str) -> String {
    internal.to_string()
}

fn write_osc52(text: &str) -> io::Result<()> {
    // OSC 52 may be ignored by some hosts; never fatal.
    let b64 = base64_encode(text.as_bytes());
    let mut out = io::stdout().lock();
    // BEL-terminated form — widely supported (Windows Terminal, xterm, …).
    write!(out, "\x1b]52;c;{b64}\x07")?;
    out.flush()
}

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    let mut i = 0;
    while i + 3 <= data.len() {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | (data[i + 2] as u32);
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push(TABLE[(n & 63) as usize] as char);
        i += 3;
    }
    match data.len() - i {
        1 => {
            let n = (data[i] as u32) << 16;
            out.push(TABLE[((n >> 18) & 63) as usize] as char);
            out.push(TABLE[((n >> 12) & 63) as usize] as char);
            out.push('=');
            out.push('=');
        }
        2 => {
            let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8);
            out.push(TABLE[((n >> 18) & 63) as usize] as char);
            out.push(TABLE[((n >> 12) & 63) as usize] as char);
            out.push(TABLE[((n >> 6) & 63) as usize] as char);
            out.push('=');
        }
        _ => {}
    }
    out
}
