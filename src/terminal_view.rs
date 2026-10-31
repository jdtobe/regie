//! The local xterm.js document and its native webview.

use crate::tabs::TabId;
use crate::terminal_protocol::UiDelivery;
use crate::theme::TerminalTheme;
use tao::dpi::{LogicalPosition, PhysicalSize};
use tao::window::Window;
use wry::{NewWindowResponse, Rect, WebView, WebViewBuilder};

const INDEX_HTML: &str = include_str!("../assets/terminal/index.html");
const XTERM_CSS: &str = include_str!("../assets/terminal/xterm.css");
const XTERM_JS: &str = include_str!("../assets/terminal/xterm.js");
const ADDON_FIT_JS: &str = include_str!("../assets/terminal/addon-fit.js");
const ADDON_WEBGL_JS: &str = include_str!("../assets/terminal/addon-webgl.js");
const REGIE_JS: &str = include_str!("../assets/terminal/regie.js");
const TABS_JS: &str = include_str!("../assets/terminal/tabs.js");
const TABS_CSS: &str = include_str!("../assets/terminal/tabs.css");

pub fn terminal_html(theme: &TerminalTheme) -> String {
    workspace_html(theme, &crate::config::Config::default())
}

pub fn workspace_html(theme: &TerminalTheme, config: &crate::config::Config) -> String {
    let mut settings: serde_json::Value =
        serde_json::from_str(&theme.config_json()).expect("terminal theme JSON");
    settings["workspace"] = true.into();
    settings["tab_position"] = serde_json::to_value(config.tab_position).unwrap();
    settings["appearance"] = serde_json::to_value(config.appearance).unwrap();
    render_html(theme, &settings.to_string())
}

fn render_html(theme: &TerminalTheme, settings: &str) -> String {
    let regie_js = REGIE_JS.replace("/*__REGIE_CONFIG__*/", settings);
    INDEX_HTML
        .replace("/*__THEME_BACKGROUND__*/", &theme.background.hex())
        .replace("/*__XTERM_CSS__*/", XTERM_CSS)
        .replace("/*__TABS_JS__*/", TABS_JS)
        .replace("/*__TABS_CSS__*/", TABS_CSS)
        .replace("/*__XTERM_JS__*/", XTERM_JS)
        .replace("/*__ADDON_FIT_JS__*/", ADDON_FIT_JS)
        .replace("/*__ADDON_WEBGL_JS__*/", ADDON_WEBGL_JS)
        .replace("/*__REGIE_JS__*/", &regie_js)
}

fn navigation_allowed(url: &str) -> bool {
    url == "about:blank" || url.starts_with("data:text/html")
}

pub struct TerminalView {
    webview: WebView,
}

impl TerminalView {
    pub fn new<F>(
        window: &Window,
        theme: &TerminalTheme,
        config: &crate::config::Config,
        ipc: F,
    ) -> Result<Self, String>
    where
        F: Fn(String) + 'static,
    {
        let webview = WebViewBuilder::new()
            .with_html(workspace_html(theme, config))
            .with_ipc_handler(move |request| ipc(request.body().clone()))
            .with_navigation_handler(|url| navigation_allowed(&url))
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
            .build(window)
            .map_err(|e| format!("could not create terminal webview: {e}"))?;
        Ok(TerminalView { webview })
    }

    pub fn focus(&self) -> Result<(), String> {
        self.webview
            .evaluate_script("window.regie?.focus();")
            .map_err(|e| format!("could not focus terminal webview: {e}"))
    }

    pub fn apply(&mut self, deliveries: &[UiDelivery]) -> Result<(), String> {
        let json = serde_json::to_string(deliveries).map_err(|e| e.to_string())?;
        self.webview
            .evaluate_script(&format!(
                "{json}.forEach(message => window.regie.apply(message));"
            ))
            .map_err(|e| format!("could not update workspace: {e}"))
    }

    pub fn focus_session(&self, id: TabId) -> Result<(), String> {
        self.webview
            .evaluate_script(&format!("window.regie?.focus({});", id.get()))
            .map_err(|e| e.to_string())
    }

    pub fn set_bounds(&self, size: PhysicalSize<u32>, scale_factor: f64) -> Result<(), String> {
        let logical = size.to_logical::<u32>(scale_factor);
        self.webview
            .set_bounds(Rect {
                position: LogicalPosition::new(0, 0).into(),
                size: logical.into(),
            })
            .map_err(|e| format!("could not resize terminal webview: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Rgb, TerminalTheme};

    #[test]
    fn terminal_document_bundles_pinned_assets_without_remote_loads() {
        let html = terminal_html(&TerminalTheme::default());
        assert!(html.contains("xterm.js 6.0.0"));
        assert!(html.contains("addon-fit 0.11.0"));
        assert!(!html.contains("<script src=\"http"));
        assert!(!html.contains("<link href=\"http"));
    }

    #[test]
    fn terminal_document_applies_the_given_theme() {
        let theme = TerminalTheme {
            background: Rgb::new(0x21, 0x21, 0x21),
            foreground: Some(Rgb::new(0xd0, 0xd0, 0xd0)),
            cursor: None,
            ansi: None,
            font_family: Some("Menlo".to_string()),
            font_size: Some(14.7),
            ..TerminalTheme::profile_fallback()
        };
        let html = terminal_html(&theme);
        assert!(html.contains(r##""foreground":"#d0d0d0""##));
        assert!(html.contains(r##""fontFamily":"Menlo""##));
        assert!(html.contains(r##""fontSize":14.7"##));
        assert!(html.contains("background: #212121"));
        assert!(
            !html.contains("__REGIE_CONFIG__"),
            "theme placeholder left behind"
        );
        assert!(
            !html.contains("__THEME_BACKGROUND__"),
            "background placeholder left behind"
        );
    }

    #[test]
    fn terminal_document_fills_the_window_and_installs_regie_bridge() {
        let html = terminal_html(&TerminalTheme::default());
        assert!(html.contains("id=\"terminal\""));
        assert!(html.contains("height: 100%"));
        assert!(html.contains("window.regie"));
        for operation in ["apply", "focus"] {
            assert!(html.contains(operation), "missing {operation}");
        }
    }

    #[test]
    fn terminal_document_uses_the_native_clipboard() {
        let html = terminal_html(&TerminalTheme::default());
        assert!(
            !html.contains("navigator.clipboard"),
            "the page is not a secure context, so it has no clipboard API"
        );
    }

    #[test]
    fn terminal_theme_delivery_updates_every_live_xterm_without_recreation() {
        let html = terminal_html(&TerminalTheme::default());
        assert!(html.contains("for (const session of sessions.values())"));
        assert!(html.contains("session.terminal.options.theme = config.theme"));
        assert!(html.contains("session.terminal.refresh"));
        assert!(!html.contains("session.terminal.clear()"));
    }

    #[test]
    fn terminal_document_loads_the_webgl_renderer_before_the_app_script() {
        let html = terminal_html(&TerminalTheme::default());
        assert!(
            html.contains("terminal.loadAddon(webgl)"),
            "the webgl renderer is not loaded"
        );
        assert!(
            html.contains("new WebglAddon.WebglAddon()"),
            "the UMD bundle exposes the addon as a namespace, not a bare constructor"
        );
        let addon = html
            .find("WebglAddon")
            .expect("the addon bundle is missing");
        let app = html
            .find("window.regie = {")
            .expect("the app script is missing");
        assert!(
            addon < app,
            "the addon must be defined before the app script uses it"
        );
    }

    #[test]
    fn navigation_allows_embedded_document_but_denies_external_urls() {
        assert!(navigation_allowed("about:blank"));
        assert!(navigation_allowed("data:text/html;charset=utf-8,terminal"));
        for url in [
            "https://example.com",
            "http://example.com",
            "file:///tmp/page.html",
        ] {
            assert!(!navigation_allowed(url), "allowed {url}");
        }
    }
}
