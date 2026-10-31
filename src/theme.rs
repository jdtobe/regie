//! The terminal's colours and font, and how they are handed to the bundled page.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Rgb {
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Rgb { red, green, blue }
    }

    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
    }

    /// AppKit reports colours as components in `0.0..=1.0`; round them to 8-bit channels.
    pub fn from_unit(red: f64, green: f64, blue: f64) -> Self {
        fn channel(component: f64) -> u8 {
            (component.clamp(0.0, 1.0) * 255.0).round() as u8
        }
        Rgb::new(channel(red), channel(green), channel(blue))
    }
}

/// Everything the page needs to look like the user's terminal. Missing parts are left to
/// xterm.js's own defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct TerminalTheme {
    pub background: Rgb,
    pub foreground: Option<Rgb>,
    pub cursor: Option<Rgb>,
    pub cursor_accent: Option<Rgb>,
    pub selection_background: Option<Rgb>,
    pub selection_foreground: Option<Rgb>,
    /// The 16 ANSI colours in xterm order: normal black to bright white.
    pub ansi: Option<[Rgb; 16]>,
    pub font_family: Option<String>,
    pub font_size: Option<f32>,
}

impl Default for TerminalTheme {
    fn default() -> Self {
        Self::regie_dark()
    }
}

#[derive(Clone, Debug, Default)]
pub struct TerminalFont {
    pub family: Option<String>,
    pub size: Option<f32>,
}

pub struct ThemeResolution {
    pub theme: TerminalTheme,
    pub notices: Vec<String>,
}

pub fn resolve(
    source: crate::config::TerminalThemeSource,
    profile: Option<TerminalTheme>,
    font: TerminalFont,
) -> ThemeResolution {
    let mut notices = vec![];
    let mut theme = match source {
        crate::config::TerminalThemeSource::Regie => TerminalTheme::regie_dark(),
        crate::config::TerminalThemeSource::Macos => profile.unwrap_or_else(|| {
            notices.push("Could not read macOS Terminal colors; using Regie Dark".into());
            TerminalTheme::regie_dark()
        }),
    };
    theme.font_family = font
        .family
        .or_else(|| TerminalTheme::profile_fallback().font_family);
    theme.font_size = font.size;
    ThemeResolution { theme, notices }
}

impl TerminalTheme {
    /// Neutral legacy fallback for missing fields in a readable Terminal profile.
    pub fn profile_fallback() -> Self {
        TerminalTheme {
            background: Rgb::new(0x1e, 0x1e, 0x1e),
            foreground: None,
            cursor: None,
            cursor_accent: None,
            selection_background: None,
            selection_foreground: None,
            ansi: None,
            font_family: Some("ui-monospace, Menlo, Monaco, monospace".to_string()),
            font_size: None,
        }
    }
    pub fn regie_dark() -> Self {
        Self {
            background: Rgb::new(0x12, 0x16, 0x15),
            foreground: Some(Rgb::new(0xdf, 0xe6, 0xdc)),
            cursor: Some(Rgb::new(0xc6, 0xd8, 0xbe)),
            cursor_accent: Some(Rgb::new(0x12, 0x16, 0x15)),
            selection_background: Some(Rgb::new(0x34, 0x4a, 0x3b)),
            selection_foreground: Some(Rgb::new(0xf2, 0xf4, 0xed)),
            ansi: Some([
                Rgb::new(0x36, 0x48, 0x3a),
                Rgb::new(0xe0, 0x8a, 0x83),
                Rgb::new(0xa8, 0xc5, 0x9a),
                Rgb::new(0xd6, 0xbf, 0x86),
                Rgb::new(0x92, 0xb4, 0xce),
                Rgb::new(0xb6, 0xa1, 0xcc),
                Rgb::new(0x8c, 0xbf, 0xba),
                Rgb::new(0xd8, 0xdf, 0xd3),
                Rgb::new(0x81, 0x90, 0x82),
                Rgb::new(0xf1, 0xaa, 0xa1),
                Rgb::new(0xc1, 0xdb, 0xaf),
                Rgb::new(0xe9, 0xd4, 0xa0),
                Rgb::new(0xb2, 0xd0, 0xe8),
                Rgb::new(0xd0, 0xbc, 0xe3),
                Rgb::new(0xac, 0xd8, 0xd1),
                Rgb::new(0xf3, 0xf4, 0xeb),
            ]),
            ..Self::profile_fallback()
        }
    }
    /// A macOS font size is in points; xterm.js takes CSS pixels, and a point is 4/3 of one.
    pub fn css_font_size(point_size: f32) -> f32 {
        (point_size * 4.0 / 3.0 * 10.0).round() / 10.0
    }

    /// The configuration object embedded in the terminal page.
    pub fn config_json(&self) -> String {
        #[derive(Serialize)]
        struct Wire<'a> {
            theme: WireTheme,
            #[serde(rename = "fontFamily", skip_serializing_if = "Option::is_none")]
            font_family: Option<&'a str>,
            #[serde(rename = "fontSize", skip_serializing_if = "Option::is_none")]
            font_size: Option<f32>,
        }

        #[derive(Serialize)]
        struct WireTheme {
            background: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            foreground: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            cursor: Option<String>,
            #[serde(rename = "cursorAccent", skip_serializing_if = "Option::is_none")]
            cursor_accent: Option<String>,
            #[serde(
                rename = "selectionBackground",
                skip_serializing_if = "Option::is_none"
            )]
            selection_background: Option<String>,
            #[serde(
                rename = "selectionForeground",
                skip_serializing_if = "Option::is_none"
            )]
            selection_foreground: Option<String>,
            #[serde(flatten)]
            ansi: std::collections::BTreeMap<&'static str, String>,
        }

        let wire = Wire {
            theme: WireTheme {
                background: self.background.hex(),
                foreground: self.foreground.map(Rgb::hex),
                cursor: self.cursor.map(Rgb::hex),
                cursor_accent: self.cursor_accent.map(Rgb::hex),
                selection_background: self.selection_background.map(Rgb::hex),
                selection_foreground: self.selection_foreground.map(Rgb::hex),
                ansi: self
                    .ansi
                    .map(|ansi| {
                        [
                            "black",
                            "red",
                            "green",
                            "yellow",
                            "blue",
                            "magenta",
                            "cyan",
                            "white",
                            "brightBlack",
                            "brightRed",
                            "brightGreen",
                            "brightYellow",
                            "brightBlue",
                            "brightMagenta",
                            "brightCyan",
                            "brightWhite",
                        ]
                        .into_iter()
                        .zip(ansi.map(Rgb::hex))
                        .collect()
                    })
                    .unwrap_or_default(),
            },
            font_family: self.font_family.as_deref(),
            font_size: self.font_size,
        };
        serde_json::to_string(&wire).expect("a theme always serializes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AFTERGLOW: [Rgb; 16] = [
        Rgb::new(0x15, 0x15, 0x15),
        Rgb::new(0xac, 0x41, 0x42),
        Rgb::new(0x7e, 0x8e, 0x50),
        Rgb::new(0xe5, 0xb5, 0x67),
        Rgb::new(0x6c, 0x99, 0xbb),
        Rgb::new(0x9f, 0x4e, 0x85),
        Rgb::new(0x7d, 0xd6, 0xcf),
        Rgb::new(0xd0, 0xd0, 0xd0),
        Rgb::new(0x50, 0x50, 0x50),
        Rgb::new(0xac, 0x41, 0x42),
        Rgb::new(0x7e, 0x8e, 0x50),
        Rgb::new(0xe5, 0xb5, 0x67),
        Rgb::new(0x6c, 0x99, 0xbb),
        Rgb::new(0x9f, 0x4e, 0x85),
        Rgb::new(0x7d, 0xd6, 0xcf),
        Rgb::new(0xf5, 0xf5, 0xf5),
    ];

    #[test]
    fn rgb_formats_as_lowercase_hex() {
        assert_eq!(Rgb::new(0x21, 0x21, 0x21).hex(), "#212121");
        assert_eq!(Rgb::new(0, 10, 255).hex(), "#000aff");
    }

    #[test]
    fn unit_components_are_rounded_and_clamped_to_eight_bits() {
        assert_eq!(Rgb::from_unit(0.0, 1.0, 0.5), Rgb::new(0, 255, 128));
        assert_eq!(Rgb::from_unit(-0.2, 1.7, 0.13), Rgb::new(0, 255, 33));
    }

    #[test]
    fn points_convert_to_rounded_css_pixels() {
        assert_eq!(TerminalTheme::css_font_size(11.0), 14.7);
        assert_eq!(TerminalTheme::css_font_size(12.0), 16.0);
    }

    #[test]
    fn default_theme_uses_all_regie_xterm_color_slots() {
        let wire: serde_json::Value =
            serde_json::from_str(&TerminalTheme::default().config_json()).unwrap();
        for (key, value) in [
            ("background", "#121615"),
            ("foreground", "#dfe6dc"),
            ("cursor", "#c6d8be"),
            ("cursorAccent", "#121615"),
            ("selectionBackground", "#344a3b"),
            ("selectionForeground", "#f2f4ed"),
            ("black", "#36483a"),
            ("red", "#e08a83"),
            ("green", "#a8c59a"),
            ("yellow", "#d6bf86"),
            ("blue", "#92b4ce"),
            ("magenta", "#b6a1cc"),
            ("cyan", "#8cbfba"),
            ("white", "#d8dfd3"),
            ("brightBlack", "#819082"),
            ("brightRed", "#f1aaa1"),
            ("brightGreen", "#c1dbaf"),
            ("brightYellow", "#e9d4a0"),
            ("brightBlue", "#b2d0e8"),
            ("brightMagenta", "#d0bce3"),
            ("brightCyan", "#acd8d1"),
            ("brightWhite", "#f3f4eb"),
        ] {
            assert_eq!(wire["theme"][key], value, "{key}");
        }
        assert!(wire["theme"].get("ansi").is_none());
    }

    #[test]
    fn resolve_preserves_font_in_both_sources() {
        use crate::config::TerminalThemeSource::{Macos, Regie};
        let profile = TerminalTheme {
            background: Rgb::new(0x21, 0x21, 0x21),
            ..TerminalTheme::profile_fallback()
        };
        for (source, expected) in [(Regie, "#121615"), (Macos, "#212121")] {
            let resolved = resolve(
                source,
                Some(profile.clone()),
                TerminalFont {
                    family: Some("Menlo".into()),
                    size: Some(14.7),
                },
            );
            assert_eq!(resolved.theme.background.hex(), expected);
            assert_eq!(resolved.theme.font_family.as_deref(), Some("Menlo"));
            assert_eq!(resolved.theme.font_size, Some(14.7));
            assert!(resolved.notices.is_empty());
        }
    }

    #[test]
    fn macos_partial_profile_has_no_regie_selection_or_cursor_accent() {
        let resolved = resolve(
            crate::config::TerminalThemeSource::Macos,
            Some(TerminalTheme::profile_fallback()),
            TerminalFont::default(),
        );
        let wire: serde_json::Value = serde_json::from_str(&resolved.theme.config_json()).unwrap();
        assert_eq!(wire["theme"]["background"], "#1e1e1e");
        for key in [
            "cursorAccent",
            "selectionBackground",
            "selectionForeground",
            "red",
        ] {
            assert!(wire["theme"].get(key).is_none());
        }
    }

    #[test]
    fn missing_macos_profile_warns_and_uses_regie_colors() {
        let resolved = resolve(
            crate::config::TerminalThemeSource::Macos,
            None,
            TerminalFont::default(),
        );
        assert_eq!(resolved.theme.background.hex(), "#121615");
        assert!(resolved.notices.join(" ").contains("Regie Dark"));
        assert!(resolved.theme.font_family.unwrap().contains("monospace"));
        assert_eq!(resolved.theme.font_size, None);
    }

    #[test]
    fn regie_colors_survive_missing_color_profile_with_readable_font() {
        let resolved = resolve(
            crate::config::TerminalThemeSource::Regie,
            None,
            TerminalFont {
                family: Some("Menlo".into()),
                size: Some(14.7),
            },
        );
        assert_eq!(
            resolved.theme,
            TerminalTheme {
                font_family: Some("Menlo".into()),
                font_size: Some(14.7),
                ..TerminalTheme::regie_dark()
            }
        );
        assert!(resolved.notices.is_empty());
    }

    #[test]
    fn profile_theme_carries_colours_font_and_palette() {
        let theme = TerminalTheme {
            background: Rgb::new(0x21, 0x21, 0x21),
            foreground: Some(Rgb::new(0xd0, 0xd0, 0xd0)),
            cursor: Some(Rgb::new(0xd0, 0xd0, 0xd0)),
            ansi: Some(AFTERGLOW),
            font_family: Some("Menlo".to_string()),
            font_size: Some(14.7),
            ..TerminalTheme::profile_fallback()
        };
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&theme.config_json()).unwrap(),
            serde_json::from_str::<serde_json::Value>(concat!(
                r##"{"theme":{"background":"#212121","foreground":"#d0d0d0","cursor":"#d0d0d0","##,
                r##""black":"#151515","red":"#ac4142","green":"#7e8e50","yellow":"#e5b567","blue":"#6c99bb","magenta":"#9f4e85","cyan":"#7dd6cf","white":"#d0d0d0","##,
                r##""brightBlack":"#505050","brightRed":"#ac4142","brightGreen":"#7e8e50","brightYellow":"#e5b567","brightBlue":"#6c99bb","brightMagenta":"#9f4e85","brightCyan":"#7dd6cf","brightWhite":"#f5f5f5"},"##,
                r##""fontFamily":"Menlo","fontSize":14.7}"##
            )).unwrap()
        );
    }

    #[test]
    fn font_family_is_quoted_as_json() {
        let theme = TerminalTheme {
            font_family: Some("Weird \"Font\"".to_string()),
            ..TerminalTheme::default()
        };
        assert!(theme
            .config_json()
            .contains(r#""fontFamily":"Weird \"Font\"""#));
    }
}
