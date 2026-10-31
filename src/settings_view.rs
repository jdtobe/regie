//! Bundled Settings document and its strictly typed webview boundary.
use crate::settings_store::{SettingKey, SettingValue};
use serde::{Deserialize, Serialize};
use tao::{
    event_loop::EventLoopWindowTarget,
    window::{Window, WindowBuilder, WindowId},
};
use wry::{NewWindowResponse, WebView, WebViewBuilder};

const HTML: &str = include_str!("../assets/settings/index.html");
const CSS: &str = include_str!("../assets/settings/settings.css");
const JS: &str = include_str!("../assets/settings/settings.js");

pub fn settings_html() -> String {
    HTML.replace(
        "<link rel=\"stylesheet\" href=\"settings.css\">",
        &format!("<style>{CSS}</style>"),
    )
    .replace(
        "<script src=\"settings.js\"></script>",
        &format!("<script>{JS}</script>"),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SettingsRequest {
    Ready,
    Edit {
        key: String,
        value: serde_json::Value,
    },
    ResetOne {
        key: String,
    },
    ResetAll,
    OpenFile,
    Close,
    Retry,
}

pub fn decode_key(key: &str) -> Result<SettingKey, String> {
    match key {
        "hotkey" => Ok(SettingKey::Hotkey),
        "height_percent" => Ok(SettingKey::HeightPercent),
        "width_percent" => Ok(SettingKey::WidthPercent),
        "hide_on_focus_lost" => Ok(SettingKey::HideOnFocusLost),
        "animation_ms" => Ok(SettingKey::AnimationMs),
        "tab_position" => Ok(SettingKey::TabPosition),
        "new_tab_directory" => Ok(SettingKey::NewTabDirectory),
        "startup_tabs" => Ok(SettingKey::StartupTabs),
        "last_tab_action" => Ok(SettingKey::LastTabAction),
        "appearance" => Ok(SettingKey::Appearance),
        "terminal_theme" => Ok(SettingKey::TerminalTheme),
        _ => Err(format!("unknown setting {key}")),
    }
}

pub fn decode_value(key: SettingKey, value: serde_json::Value) -> Result<SettingValue, String> {
    macro_rules! convert {
        ($variant:ident) => {
            serde_json::from_value(value)
                .map(SettingValue::$variant)
                .map_err(|e| e.to_string())
        };
    }
    match key {
        SettingKey::Hotkey => convert!(String),
        SettingKey::HeightPercent | SettingKey::WidthPercent => convert!(U8),
        SettingKey::AnimationMs => convert!(U32),
        SettingKey::HideOnFocusLost => convert!(Bool),
        SettingKey::TabPosition => convert!(TabPosition),
        SettingKey::NewTabDirectory => convert!(NewTabDirectory),
        SettingKey::StartupTabs => convert!(StartupTabs),
        SettingKey::LastTabAction => convert!(LastTabAction),
        SettingKey::Appearance => convert!(Appearance),
        SettingKey::TerminalTheme => convert!(TerminalTheme),
    }
}

pub fn parse_request(body: &str) -> Result<SettingsRequest, String> {
    let raw: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let object = raw
        .as_object()
        .ok_or("settings request must be an object")?;
    let kind = object
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or("missing request type")?;
    let allowed: &[&str] = match kind {
        "edit" => &["type", "key", "value"],
        "reset_one" => &["type", "key"],
        "reset_all" | "open_file" | "close" | "retry" | "ready" => &["type"],
        _ => return Err("unknown settings request".into()),
    };
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("unexpected settings request field".into());
    }
    let request: SettingsRequest = serde_json::from_value(raw).map_err(|e| e.to_string())?;
    match &request {
        SettingsRequest::Edit { key, value } => {
            decode_value(decode_key(key)?, value.clone())?;
        }
        SettingsRequest::ResetOne { key } => {
            decode_key(key)?;
        }
        _ => {}
    }
    Ok(request)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SettingsFieldView {
    pub key: String,
    pub value: Option<serde_json::Value>,
    pub default: serde_json::Value,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SettingsViewState {
    pub fields: Vec<SettingsFieldView>,
    pub appearance: String,
    pub status: Option<String>,
    pub warning: Option<String>,
    pub retryable: bool,
    pub can_retry: bool,
}

fn json_value(value: SettingValue) -> serde_json::Value {
    match value {
        SettingValue::String(v) => serde_json::json!(v),
        SettingValue::U8(v) => serde_json::json!(v),
        SettingValue::U32(v) => serde_json::json!(v),
        SettingValue::Bool(v) => serde_json::json!(v),
        SettingValue::TabPosition(v) => serde_json::json!(v),
        SettingValue::NewTabDirectory(v) => serde_json::json!(v),
        SettingValue::StartupTabs(v) => serde_json::json!(v),
        SettingValue::LastTabAction(v) => serde_json::json!(v),
        SettingValue::Appearance(v) => serde_json::json!(v),
        SettingValue::TerminalTheme(v) => serde_json::json!(v),
    }
}

pub fn view_state(
    controller: &crate::settings::SettingsController,
    warning: Option<String>,
) -> SettingsViewState {
    let warning = if matches!(
        controller.snapshot().status,
        crate::settings_store::ConfigStatus::Invalid
    ) {
        crate::config::Config::from_toml_str(&controller.snapshot().text)
            .err()
            .or(warning)
    } else {
        warning
    };
    let defaults = crate::config::Config::default();
    let fields = controller
        .snapshot()
        .fields
        .iter()
        .map(|field| SettingsFieldView {
            key: field.key.name().into(),
            value: field.value.clone().map(json_value),
            default: json_value(
                crate::settings::value_for(&defaults, field.key)
                    .expect("every setting has a default"),
            ),
            error: field.error.clone(),
        })
        .collect();
    SettingsViewState {
        fields,
        appearance: serde_json::to_value(controller.active().appearance)
            .unwrap()
            .as_str()
            .unwrap()
            .into(),
        status: match &controller.snapshot().status {
            crate::settings_store::ConfigStatus::Valid(config) if config != controller.active() => {
                Some("Saved settings are not currently active.".into())
            }
            crate::settings_store::ConfigStatus::Valid(_) => None,
            _ => Some("Changes will apply once the configuration is valid.".into()),
        },
        warning,
        retryable: controller.has_retryable_changes(),
        can_retry: controller.can_retry(),
    }
}

pub struct SettingsView {
    window: Window,
    webview: WebView,
}

impl SettingsView {
    pub fn new<T: 'static, F>(
        target: &EventLoopWindowTarget<T>,
        on_request: F,
    ) -> Result<Self, String>
    where
        F: Fn(SettingsRequest) + 'static,
    {
        let window = WindowBuilder::new()
            .with_title("Settings")
            .with_visible(false)
            .with_inner_size(tao::dpi::LogicalSize::new(900.0, 680.0))
            .build(target)
            .map_err(|e| format!("could not create Settings window: {e}"))?;
        #[cfg(target_os = "macos")]
        {
            use tao::platform::macos::WindowExtMacOS;
            // SAFETY: tao owns the live NSWindow for the lifetime of `window`.
            let native = unsafe { &*window.ns_window().cast::<objc2_app_kit::NSWindow>() };
            native.setLevel(25);
        }
        let webview = WebViewBuilder::new()
            .with_html(settings_html())
            .with_ipc_handler(move |request| {
                if let Ok(message) = parse_request(request.body()) {
                    on_request(message);
                }
            })
            .with_navigation_handler(|url| {
                url == "about:blank" || url.starts_with("data:text/html")
            })
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
            .build(&window)
            .map_err(|e| format!("could not create Settings webview: {e}"))?;
        Ok(Self { window, webview })
    }
    pub fn show(&self) {
        self.window.set_visible(true);
        #[cfg(target_os = "macos")]
        if let Some(marker) = objc2::MainThreadMarker::new() {
            objc2_app_kit::NSApplication::sharedApplication(marker).activate();
        }
        self.window.set_focus();
    }
    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }
    pub fn hide(&self) {
        self.window.set_visible(false);
    }
    pub fn is_focused(&self) -> bool {
        self.window.is_focused()
    }
    pub fn render(&self, state: &SettingsViewState) -> Result<(), String> {
        let json = serde_json::to_string(state)
            .map_err(|e| format!("could not serialize Settings state: {e}"))?;
        let script = format!("window.settings.render({json});");
        self.webview
            .evaluate_script(&script)
            .map_err(|e| format!("could not render Settings: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_request_rejects_unknown_keys_and_wrong_value_types() {
        assert!(parse_request(r#"{"type":"edit","key":"height_percent","value":40}"#).is_ok());
        assert!(parse_request(r#"{"type":"edit","key":"height_percent","value":"40"}"#).is_err());
        assert!(parse_request(r#"{"type":"reset_one","key":"unknown"}"#).is_err());
        assert!(parse_request(r#"{"type":"retry","extra":true}"#).is_err());
    }

    #[test]
    fn view_state_exposes_defaults_and_invalid_fields_without_hiding_them() {
        let paths =
            crate::app_paths::AppPaths::new(std::path::Path::new("/nonexistent-regie-test"));
        let mut snapshot = crate::settings_store::SettingsStore::read(&paths);
        snapshot.fields[0].value = None;
        snapshot.fields[0].error = Some("bad hotkey".into());
        let controller =
            crate::settings::SettingsController::new(snapshot, crate::config::Config::default());
        let state = view_state(&controller, None);
        assert_eq!(state.fields.len(), 11);
        assert_eq!(state.fields[0].error.as_deref(), Some("bad hotkey"));
        assert_eq!(state.fields[0].value, None);
        assert_eq!(state.fields[1].default, serde_json::json!(40));
    }

    #[test]
    fn invalid_config_explains_when_edits_will_apply() {
        let paths =
            crate::app_paths::AppPaths::new(std::path::Path::new("/nonexistent-regie-test"));
        let mut snapshot = crate::settings_store::SettingsStore::read(&paths);
        snapshot.status = crate::settings_store::ConfigStatus::Invalid;
        let controller =
            crate::settings::SettingsController::new(snapshot, crate::config::Config::default());
        let state = view_state(&controller, Some("Invalid config".into()));
        assert!(state
            .status
            .unwrap()
            .contains("once the configuration is valid"));
    }

    #[test]
    fn invalid_config_shows_the_parser_error() {
        let paths =
            crate::app_paths::AppPaths::new(std::path::Path::new("/nonexistent-regie-test"));
        let mut snapshot = crate::settings_store::SettingsStore::read(&paths);
        snapshot.text = "unknown_setting = true".into();
        snapshot.status = crate::settings_store::ConfigStatus::Invalid;
        let controller =
            crate::settings::SettingsController::new(snapshot, crate::config::Config::default());
        let state = view_state(&controller, Some("Invalid config".into()));
        assert!(state.warning.unwrap().contains("unknown_setting"));
    }
}
