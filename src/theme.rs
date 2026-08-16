//! Built-in palettes for `Theme <Name>` on a Window, Screen, or Page.
//!
//! Iced supplies most of the names (`Dracula`, `Nord`, …). `NightOwl` and
//! `JellyFish` are Bust's own — they lower to `iced::Theme::custom` on a Window,
//! CSS variables on a Page, and ratatui RGB on a Screen.

pub struct Spec {
    /// Canonical PascalCase name (`NightOwl`, `CatppuccinMocha`).
    pub name: &'static str,
    /// Iced enum variant, or `None` for a custom palette (`NightOwl`, `JellyFish`).
    pub iced: Option<&'static str>,
    pub background: (u8, u8, u8),
    pub text: (u8, u8, u8),
    pub primary: (u8, u8, u8),
    pub success: (u8, u8, u8),
    pub danger: (u8, u8, u8),
    /// Chart series; `None` picks a dark or light rainbow from the background.
    series: Option<[(u8, u8, u8); 6]>,
}

const DARK_SERIES: [(u8, u8, u8); 6] = [
    (139, 233, 253),
    (241, 250, 140),
    (80, 250, 123),
    (255, 121, 198),
    (255, 85, 85),
    (130, 170, 255),
];

const LIGHT_SERIES: [(u8, u8, u8); 6] = [
    (0, 95, 135),
    (175, 135, 0),
    (95, 135, 0),
    (175, 0, 95),
    (215, 0, 0),
    (0, 95, 175),
];

/// Every theme a `Theme` line may name, in help / error-message order.
pub static THEMES: &[Spec] = &[
    spec("Light", Some("Light"), (255, 255, 255), (0, 0, 0), (94, 124, 226)),
    spec("Dark", Some("Dark"), (32, 34, 37), (230, 230, 230), (94, 124, 226)),
    spec("Dracula", Some("Dracula"), (40, 42, 54), (248, 248, 242), (189, 147, 249)),
    spec("Nord", Some("Nord"), (46, 52, 64), (236, 239, 244), (143, 188, 187)),
    spec("SolarizedLight", Some("SolarizedLight"), (253, 246, 227), (101, 123, 131), (42, 161, 152)),
    spec("SolarizedDark", Some("SolarizedDark"), (0, 43, 54), (131, 148, 150), (42, 161, 152)),
    spec("GruvboxLight", Some("GruvboxLight"), (251, 241, 199), (40, 40, 40), (69, 133, 136)),
    spec("GruvboxDark", Some("GruvboxDark"), (40, 40, 40), (251, 241, 199), (69, 133, 136)),
    spec("CatppuccinLatte", Some("CatppuccinLatte"), (239, 241, 245), (76, 79, 105), (30, 102, 245)),
    spec("CatppuccinFrappe", Some("CatppuccinFrappe"), (48, 52, 70), (198, 208, 245), (140, 170, 238)),
    spec("CatppuccinMacchiato", Some("CatppuccinMacchiato"), (36, 39, 58), (202, 211, 245), (138, 173, 244)),
    spec("CatppuccinMocha", Some("CatppuccinMocha"), (30, 30, 46), (205, 214, 244), (137, 180, 250)),
    spec("TokyoNight", Some("TokyoNight"), (26, 27, 38), (154, 165, 206), (42, 195, 222)),
    spec("TokyoNightStorm", Some("TokyoNightStorm"), (36, 40, 59), (154, 165, 206), (42, 195, 222)),
    spec("TokyoNightLight", Some("TokyoNightLight"), (213, 214, 219), (86, 90, 110), (22, 103, 117)),
    spec("KanagawaWave", Some("KanagawaWave"), (54, 54, 70), (220, 215, 186), (45, 79, 103)),
    spec("KanagawaDragon", Some("KanagawaDragon"), (24, 22, 22), (197, 201, 197), (34, 50, 73)),
    spec("KanagawaLotus", Some("KanagawaLotus"), (242, 236, 188), (84, 84, 100), (201, 203, 209)),
    spec("Moonfly", Some("Moonfly"), (8, 8, 8), (189, 189, 189), (128, 160, 255)),
    spec("Nightfly", Some("Nightfly"), (1, 22, 39), (189, 193, 198), (130, 170, 255)),
    spec("Oxocarbon", Some("Oxocarbon"), (35, 35, 35), (208, 208, 208), (0, 180, 255)),
    spec("Ferra", Some("Ferra"), (43, 41, 45), (254, 205, 178), (209, 209, 224)),
    // Sarah Drasner's Night Owl — not an Iced built-in.
    Spec {
        name: "NightOwl",
        iced: None,
        background: (1, 22, 39),
        text: (214, 222, 235),
        primary: (130, 170, 255),
        success: (173, 219, 103),
        danger: (239, 83, 80),
        series: Some([
            (130, 170, 255),
            (173, 219, 103),
            (199, 146, 234),
            (127, 219, 202),
            (247, 140, 108),
            (255, 88, 116),
        ]),
    },
    // Bioluminescent ocean — Bust's own, not an Iced built-in.
    Spec {
        name: "JellyFish",
        iced: None,
        background: (11, 19, 43),
        text: (234, 246, 255),
        primary: (255, 110, 180),
        success: (94, 234, 212),
        danger: (255, 93, 143),
        series: Some([
            (77, 238, 234),
            (255, 110, 180),
            (199, 125, 255),
            (128, 255, 219),
            (255, 209, 102),
            (247, 37, 133),
        ]),
    },
];

const fn spec(
    name: &'static str,
    iced: Option<&'static str>,
    background: (u8, u8, u8),
    text: (u8, u8, u8),
    primary: (u8, u8, u8),
) -> Spec {
    Spec {
        name,
        iced,
        background,
        text,
        primary,
        success: (80, 250, 123),
        danger: (255, 85, 85),
        series: None,
    }
}

fn key(name: &str) -> String {
    name.to_ascii_lowercase().replace('_', "")
}

/// The spec for `Theme <name>`, matched case-insensitively; underscores optional
/// (`Night_Owl` = `NightOwl`).
pub fn lookup(name: &str) -> Option<&'static Spec> {
    let k = key(name);
    THEMES.iter().find(|t| key(t.name) == k)
}

pub fn names() -> Vec<&'static str> {
    THEMES.iter().map(|t| t.name).collect()
}

impl Spec {
    pub fn hex_bg(&self) -> String {
        hex(self.background)
    }
    pub fn hex_text(&self) -> String {
        hex(self.text)
    }
    pub fn hex_primary(&self) -> String {
        hex(self.primary)
    }

    /// Text colour that stays readable on `primary` (status bar, menu, dialog).
    pub fn chrome_fg(&self) -> (u8, u8, u8) {
        if luminance(self.primary) > 140.0 {
            self.background
        } else {
            self.text
        }
    }

    pub fn series_colors(&self) -> [(u8, u8, u8); 6] {
        if let Some(s) = self.series {
            return s;
        }
        if luminance(self.background) > 140.0 {
            LIGHT_SERIES
        } else {
            DARK_SERIES
        }
    }

    /// `iced::Theme::Dracula` or `iced::Theme::custom(...)` for NightOwl /
    /// JellyFish (`custom_with_fn` so JellyFish primary-button text stays ink).
    pub fn iced_expr(&self) -> String {
        if let Some(v) = self.iced {
            return format!("iced::Theme::{v}");
        }
        let (b, t, p, s, d) = (
            rgb8(self.background),
            rgb8(self.text),
            rgb8(self.primary),
            rgb8(self.success),
            rgb8(self.danger),
        );
        let generate = if self.name == "JellyFish" {
            // Iced's contrast pick for this pink is a dark-pink that washes out
            // on the selected tab / primary button. Ink stays readable.
            ", |p| { let mut e = iced::theme::palette::Extended::generate(p); \
             let ink = iced::Color::from_rgb8(16, 14, 22); \
             e.primary.base.text = ink; e.primary.strong.text = ink; e.primary.weak.text = ink; e }"
        } else {
            ""
        };
        let ctor = if generate.is_empty() { "custom" } else { "custom_with_fn" };
        format!(
            "iced::Theme::{ctor}(String::from({:?}), iced::theme::Palette {{ \
             background: {b}, text: {t}, primary: {p}, success: {s}, danger: {d} }}{generate})",
            pretty_name(self.name)
        )
    }
}

fn pretty_name(canon: &str) -> &'static str {
    match canon {
        "NightOwl" => "Night Owl",
        "JellyFish" => "JellyFish",
        _ => "Custom",
    }
}

fn hex((r, g, b): (u8, u8, u8)) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn rgb8((r, g, b): (u8, u8, u8)) -> String {
    format!("iced::Color::from_rgb8({r}, {g}, {b})")
}

fn luminance((r, g, b): (u8, u8, u8)) -> f32 {
    0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32
}

/// Ratatui `Color::Rgb(...)` for a triple.
pub fn ratatui_rgb((r, g, b): (u8, u8, u8)) -> String {
    format!("ratatui::style::Color::Rgb({r}, {g}, {b})")
}
