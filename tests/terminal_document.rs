use regie::config::{Appearance, Config, TerminalThemeSource};
use regie::terminal_view::workspace_html;
use regie::theme::{resolve, Rgb, TerminalFont, TerminalTheme};

#[test]
fn production_documents_keep_palette_source_separate_from_ui_appearance() {
    let output = std::env::var_os("REGIE_TERMINAL_FIXTURE_DIR").map(std::path::PathBuf::from);
    if let Some(path) = &output {
        std::fs::create_dir_all(path).unwrap();
    }
    for (source_name, source, background) in [
        ("regie", TerminalThemeSource::Regie, "#121615"),
        ("macos", TerminalThemeSource::Macos, "#212121"),
    ] {
        for (appearance_name, appearance) in [
            ("system", Appearance::System),
            ("light", Appearance::Light),
            ("dark", Appearance::Dark),
        ] {
            let profile = TerminalTheme {
                background: Rgb::new(0x21, 0x21, 0x21),
                foreground: Some(Rgb::new(0xd0, 0xd0, 0xd0)),
                cursor: Some(Rgb::new(0xee, 0xcc, 0x88)),
                ..TerminalTheme::profile_fallback()
            };
            let theme = resolve(
                source,
                Some(profile),
                TerminalFont {
                    family: Some("Menlo".into()),
                    size: Some(16.0),
                },
            )
            .theme;
            let options: serde_json::Value = serde_json::from_str(&theme.config_json()).unwrap();
            assert_eq!(options["theme"]["background"], background);
            assert_eq!(options["fontFamily"], "Menlo");
            assert_eq!(options["fontSize"], 16.0);
            assert!(options["theme"].get("ansi").is_none());
            if source == TerminalThemeSource::Regie {
                assert_eq!(options["theme"]["red"], "#e08a83");
            } else {
                assert!(options["theme"].get("selectionBackground").is_none());
            }
            let html = workspace_html(
                &theme,
                &Config {
                    terminal_theme: source,
                    appearance,
                    ..Config::default()
                },
            );
            assert!(html.contains(&format!("background: {background}")));
            assert!(html.contains(&format!("\"appearance\":\"{appearance_name}\"")));
            assert!(!html.contains("/*__REGIE_"));
            if let Some(path) = &output {
                std::fs::write(
                    path.join(format!("{source_name}-{appearance_name}.html")),
                    html,
                )
                .unwrap();
            }
        }
    }
}
