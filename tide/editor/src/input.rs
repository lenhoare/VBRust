//! Backend-agnostic input events for the editor.

/// Modifier keys held with a key press.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyMods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl KeyMods {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn ctrl() -> Self {
        Self {
            ctrl: true,
            ..Self::default()
        }
    }

    pub fn shift() -> Self {
        Self {
            shift: true,
            ..Self::default()
        }
    }

    pub fn ctrl_shift() -> Self {
        Self {
            ctrl: true,
            shift: true,
            ..Self::default()
        }
    }
}

/// Keys the editor understands. Hosts map terminal/backend events into these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Delete,
    Tab,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Esc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    pub mods: KeyMods,
}

impl KeyEvent {
    pub fn new(key: Key, mods: KeyMods) -> Self {
        Self { key, mods }
    }

    pub fn char(c: char) -> Self {
        Self::new(Key::Char(c), KeyMods::none())
    }
}
