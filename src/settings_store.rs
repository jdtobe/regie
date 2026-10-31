//! Lossless, field-oriented access to the user's TOML settings.
use crate::{
    app_paths::{self, AppPaths},
    config::{
        self, Appearance, Config, LastTabAction, NewTabDirectory, StartupTabs, TabPosition,
        TerminalThemeSource,
    },
};
use std::{fs, io::Write, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingKey {
    Hotkey,
    HeightPercent,
    WidthPercent,
    HideOnFocusLost,
    AnimationMs,
    TabPosition,
    NewTabDirectory,
    StartupTabs,
    LastTabAction,
    Appearance,
    TerminalTheme,
}
impl SettingKey {
    pub const ALL: [Self; 11] = [
        Self::Hotkey,
        Self::HeightPercent,
        Self::WidthPercent,
        Self::HideOnFocusLost,
        Self::AnimationMs,
        Self::TabPosition,
        Self::NewTabDirectory,
        Self::StartupTabs,
        Self::LastTabAction,
        Self::Appearance,
        Self::TerminalTheme,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Hotkey => "hotkey",
            Self::HeightPercent => "height_percent",
            Self::WidthPercent => "width_percent",
            Self::HideOnFocusLost => "hide_on_focus_lost",
            Self::AnimationMs => "animation_ms",
            Self::TabPosition => "tab_position",
            Self::NewTabDirectory => "new_tab_directory",
            Self::StartupTabs => "startup_tabs",
            Self::LastTabAction => "last_tab_action",
            Self::Appearance => "appearance",
            Self::TerminalTheme => "terminal_theme",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingValue {
    String(String),
    U8(u8),
    U32(u32),
    Bool(bool),
    TabPosition(TabPosition),
    NewTabDirectory(NewTabDirectory),
    StartupTabs(StartupTabs),
    LastTabAction(LastTabAction),
    Appearance(Appearance),
    TerminalTheme(TerminalThemeSource),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldState {
    pub key: SettingKey,
    pub value: Option<SettingValue>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigStatus {
    Valid(Config),
    Invalid,
    Malformed(String),
    Unreadable(String),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSnapshot {
    pub path: PathBuf,
    pub text: String,
    pub fields: Vec<FieldState>,
    pub status: ConfigStatus,
}
#[derive(Debug)]
pub enum StoreError {
    Conflict(SettingKey),
    Malformed(String),
    Inaccessible(String),
    FailedWrite(String),
}
pub struct SettingsStore;

impl SettingsStore {
    pub fn read(paths: &AppPaths) -> ConfigSnapshot {
        let path = paths.config.clone();
        let (source, bytes) = match fs::read(&path) {
            Ok(b) => (path.clone(), b),
            Err(e)
                if e.kind() == std::io::ErrorKind::NotFound
                    && matches!(fs::symlink_metadata(&path), Err(ref m) if m.kind() == std::io::ErrorKind::NotFound) =>
            {
                match app_paths::read_preferred(&path, &paths.legacy_config) {
                    Ok(Some(file)) => (file.path, file.bytes),
                    Ok(None) => (path.clone(), Vec::new()),
                    Err(e) => return unreadable(path, e),
                }
            }
            Err(e) => {
                return unreadable(
                    path.clone(),
                    format!("could not read {}: {e}", path.display()),
                )
            }
        };
        let text = match String::from_utf8(bytes) {
            Ok(t) => t,
            Err(e) => return unreadable(source, format!("invalid UTF-8: {e}")),
        };
        let fields = fields(&text);
        let status = match Config::from_toml_str(&text) {
            Ok(c) => ConfigStatus::Valid(c),
            Err(e) if text.parse::<toml_edit::DocumentMut>().is_err() => ConfigStatus::Malformed(e),
            Err(_) => ConfigStatus::Invalid,
        };
        ConfigSnapshot {
            path: source,
            text,
            fields,
            status,
        }
    }

    /// Saves selected fields after rechecking the effective source and bytes directly before
    /// replacement. Filesystem APIs do not provide a portable compare-and-rename, so an arbitrary
    /// external editor can still race the final check and rename.
    pub fn write(
        paths: &AppPaths,
        expected: &ConfigSnapshot,
        changes: &[(SettingKey, SettingValue)],
    ) -> Result<ConfigSnapshot, StoreError> {
        Self::write_with_hook(paths, expected, changes, || {})
    }

    fn write_with_hook(
        paths: &AppPaths,
        expected: &ConfigSnapshot,
        changes: &[(SettingKey, SettingValue)],
        before_replace: impl FnOnce(),
    ) -> Result<ConfigSnapshot, StoreError> {
        let path = &paths.config;
        let original_mode = canonical_mode(path)?;
        if let ConfigStatus::Unreadable(error) = &expected.status {
            return Err(StoreError::Inaccessible(error.clone()));
        }
        if let ConfigStatus::Malformed(error) = &expected.status {
            return Err(StoreError::Malformed(error.clone()));
        }
        let current = Self::read(paths);
        if let ConfigStatus::Unreadable(error) = &current.status {
            return Err(StoreError::Inaccessible(error.clone()));
        }
        if current.path != expected.path {
            return Err(StoreError::Conflict(
                changes
                    .first()
                    .map(|change| change.0)
                    .unwrap_or(SettingKey::Hotkey),
            ));
        }
        let actual = current.text;
        let canonical_present = original_mode.is_some();
        let mut doc = actual
            .parse::<toml_edit::DocumentMut>()
            .map_err(|e| StoreError::Malformed(e.to_string()))?;
        let empty = || toml::Value::Table(Default::default());
        let expected_values =
            toml::from_str::<toml::Value>(&expected.text).unwrap_or_else(|_| empty());
        let actual_values = toml::from_str::<toml::Value>(&actual).unwrap_or_else(|_| empty());
        for (key, value) in changes {
            if actual_values.get(key.name()) != expected_values.get(key.name()) {
                return Err(StoreError::Conflict(*key));
            }
            let encoded = encode(value).ok_or_else(|| {
                StoreError::Malformed(format!("wrong value type for {}", key.name()))
            })?;
            // Parse an isolated field via the project's parser, including normal validation.
            let check = format!("{} = {}", key.name(), encoded);
            let parsed: toml::Value =
                toml::from_str(&check).map_err(|e| StoreError::Malformed(e.to_string()))?;
            decode(*key, parsed.get(key.name()).unwrap()).map_err(StoreError::Malformed)?;
            let mut replacement = encoded
                .parse::<toml_edit::Value>()
                .map_err(|e| StoreError::Malformed(e.to_string()))?;
            if let Some(old) = doc.get(key.name()).and_then(toml_edit::Item::as_value) {
                replacement.decor_mut().clone_from(old.decor());
            }
            doc[key.name()] = replacement.into();
        }
        let updated = doc.to_string();
        let mut temp = path.with_extension(format!("toml.{}.tmp", std::process::id()));
        let mut suffix = 0;
        while temp.exists() {
            suffix += 1;
            temp = path.with_extension(format!("toml.{}-{suffix}.tmp", std::process::id()));
        }
        let mut temp_created = false;
        let mut before_replace = Some(before_replace);
        let result = (|| -> Result<(), StoreError> {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| StoreError::FailedWrite(error.to_string()))?;
            }
            write_temp_file(&temp, updated.as_bytes(), original_mode)
                .map_err(|error| StoreError::FailedWrite(error.to_string()))?;
            temp_created = true;
            if let Some(hook) = before_replace.take() {
                hook();
            }
            let latest_canonical_mode = canonical_mode(path)?;
            if latest_canonical_mode != original_mode {
                return Err(StoreError::Conflict(
                    changes.first().map(|c| c.0).unwrap_or(SettingKey::Hotkey),
                ));
            }
            let latest = Self::read(paths);
            let unchanged = !matches!(latest.status, ConfigStatus::Unreadable(_))
                && latest.path == current.path
                && latest.text.as_bytes() == actual.as_bytes()
                && latest_canonical_mode.is_some() == canonical_present;
            if !unchanged {
                return Err(StoreError::Conflict(
                    changes.first().map(|c| c.0).unwrap_or(SettingKey::Hotkey),
                ));
            }
            fs::rename(&temp, path).map_err(|error| StoreError::FailedWrite(error.to_string()))?;
            if let Some(parent) = path.parent() {
                if let Ok(dir) = fs::File::open(parent) {
                    let _ = dir.sync_all();
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            if temp_created {
                let _ = fs::remove_file(&temp);
            }
            return Err(e);
        }
        Ok(Self::read(paths))
    }
}
fn unreadable(path: PathBuf, error: String) -> ConfigSnapshot {
    ConfigSnapshot {
        path,
        text: String::new(),
        fields: SettingKey::ALL
            .iter()
            .map(|key| FieldState {
                key: *key,
                value: None,
                error: Some(error.clone()),
            })
            .collect(),
        status: ConfigStatus::Unreadable(error),
    }
}
fn write_temp_file(path: &std::path::Path, bytes: &[u8], mode: Option<u32>) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    let write_result = file.write_all(bytes);
    #[cfg(unix)]
    let write_result = write_result.and_then(|()| {
        if let Some(mode) = mode {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(mode))?;
        }
        Ok(())
    });
    #[cfg(not(unix))]
    let write_result = write_result;
    let write_result = write_result.and_then(|()| file.sync_all());
    if let Err(error) = write_result {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error);
    }
    Ok(())
}
fn canonical_mode(path: &std::path::Path) -> Result<Option<u32>, StoreError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(StoreError::Inaccessible(format!(
                "could not inspect {}: {error}",
                path.display()
            )))
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(StoreError::Inaccessible(format!(
            "{} is a symbolic link; settings cannot be saved through it. Replace it with a regular config file or edit the link target manually.",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok(Some(metadata.permissions().mode() & 0o777))
    }
    #[cfg(not(unix))]
    {
        Ok(Some(0))
    }
}
fn fields(text: &str) -> Vec<FieldState> {
    let parsed = toml::from_str::<toml::Value>(text);
    if let Err(error) = &parsed {
        return SettingKey::ALL
            .iter()
            .map(|key| FieldState {
                key: *key,
                value: None,
                error: Some(error.to_string()),
            })
            .collect();
    }
    SettingKey::ALL
        .iter()
        .map(|key| {
            let value = parsed.as_ref().ok().and_then(|v| v.get(key.name()));
            match value.and_then(|v| decode(*key, v).ok()) {
                Some(value) => FieldState {
                    key: *key,
                    value: Some(value),
                    error: None,
                },
                None if value.is_none() => FieldState {
                    key: *key,
                    value: default(*key),
                    error: None,
                },
                None => FieldState {
                    key: *key,
                    value: None,
                    error: Some(format!("invalid {}", key.name())),
                },
            }
        })
        .collect()
}
fn default(k: SettingKey) -> Option<SettingValue> {
    let c = Config::default();
    Some(match k {
        SettingKey::Hotkey => SettingValue::String(c.hotkey),
        SettingKey::HeightPercent => SettingValue::U8(c.height_percent),
        SettingKey::WidthPercent => SettingValue::U8(c.width_percent),
        SettingKey::HideOnFocusLost => SettingValue::Bool(c.hide_on_focus_lost),
        SettingKey::AnimationMs => SettingValue::U32(c.animation_ms),
        SettingKey::TabPosition => SettingValue::TabPosition(c.tab_position),
        SettingKey::NewTabDirectory => SettingValue::NewTabDirectory(c.new_tab_directory),
        SettingKey::StartupTabs => SettingValue::StartupTabs(c.startup_tabs),
        SettingKey::LastTabAction => SettingValue::LastTabAction(c.last_tab_action),
        SettingKey::Appearance => SettingValue::Appearance(c.appearance),
        SettingKey::TerminalTheme => SettingValue::TerminalTheme(c.terminal_theme),
    })
}
fn decode(k: SettingKey, v: &toml::Value) -> Result<SettingValue, String> {
    macro_rules! get {
        ($t:ty,$variant:ident) => {
            v.clone()
                .try_into::<$t>()
                .map(SettingValue::$variant)
                .map_err(|e: toml::de::Error| e.to_string())
        };
    }
    let out = match k {
        SettingKey::Hotkey => get!(String, String),
        SettingKey::HeightPercent => get!(u8, U8),
        SettingKey::WidthPercent => get!(u8, U8),
        SettingKey::HideOnFocusLost => get!(bool, Bool),
        SettingKey::AnimationMs => get!(u32, U32),
        SettingKey::TabPosition => get!(TabPosition, TabPosition),
        SettingKey::NewTabDirectory => get!(NewTabDirectory, NewTabDirectory),
        SettingKey::StartupTabs => get!(StartupTabs, StartupTabs),
        SettingKey::LastTabAction => get!(LastTabAction, LastTabAction),
        SettingKey::Appearance => get!(Appearance, Appearance),
        SettingKey::TerminalTheme => get!(TerminalThemeSource, TerminalTheme),
    }?;
    if let SettingValue::String(ref hotkey) = out {
        crate::hotkey::parse_hotkey(hotkey)?;
    }
    if let SettingValue::U8(n) = out {
        if !config::HEIGHT_PERCENT_RANGE.contains(&n) && k == SettingKey::HeightPercent
            || !config::WIDTH_PERCENT_RANGE.contains(&n) && k == SettingKey::WidthPercent
        {
            return Err(format!("{} out of range", k.name()));
        }
        return Ok(SettingValue::U8(n));
    }
    if let SettingValue::U32(n) = out {
        if n > config::ANIMATION_MS_MAX {
            return Err("animation_ms out of range".into());
        }
        return Ok(SettingValue::U32(n));
    }
    Ok(out)
}
fn encode(v: &SettingValue) -> Option<String> {
    Some(match v {
        SettingValue::String(s) => toml::Value::String(s.clone()).to_string(),
        SettingValue::U8(n) => n.to_string(),
        SettingValue::U32(n) => n.to_string(),
        SettingValue::Bool(b) => b.to_string(),
        SettingValue::TabPosition(v) => toml::Value::try_from(v).ok()?.to_string(),
        SettingValue::NewTabDirectory(v) => toml::Value::try_from(v).ok()?.to_string(),
        SettingValue::StartupTabs(v) => toml::Value::try_from(v).ok()?.to_string(),
        SettingValue::LastTabAction(v) => toml::Value::try_from(v).ok()?.to_string(),
        SettingValue::Appearance(v) => toml::Value::try_from(v).ok()?.to_string(),
        SettingValue::TerminalTheme(v) => toml::Value::try_from(v).ok()?.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_defaults_and_recovers_fields() {
        let p = temp();
        std::fs::create_dir_all(p.config.parent().unwrap()).unwrap();
        let s = SettingsStore::read(&p);
        assert_eq!(s.fields.len(), 11);
        assert!(matches!(s.status, ConfigStatus::Valid(_)));
        std::fs::write(&p.config, "height_percent='bad'\nwidth_percent=66").unwrap();
        let s = SettingsStore::read(&p);
        assert!(s
            .fields
            .iter()
            .find(|f| f.key == SettingKey::HeightPercent)
            .unwrap()
            .error
            .is_some());
        assert_eq!(
            s.fields
                .iter()
                .find(|f| f.key == SettingKey::WidthPercent)
                .unwrap()
                .value,
            Some(SettingValue::U8(66))
        );
    }
    #[test]
    fn writes_preserving_comments_and_unrelated_keys() {
        let p = temp();
        std::fs::create_dir_all(p.config.parent().unwrap()).unwrap();
        std::fs::write(
            &p.config,
            "# keep\nheight_percent = 40 # inline\nunknown = 'x'\n",
        )
        .unwrap();
        let s = SettingsStore::read(&p);
        let out =
            SettingsStore::write(&p, &s, &[(SettingKey::HeightPercent, SettingValue::U8(70))])
                .unwrap();
        assert!(out.text.contains("# keep"));
        assert!(out.text.contains("unknown = 'x'"));
        assert!(out.text.contains("70 # inline"));
    }
    #[test]
    fn refuses_same_field_conflict() {
        let p = temp();
        std::fs::create_dir_all(p.config.parent().unwrap()).unwrap();
        std::fs::write(&p.config, "height_percent=40").unwrap();
        let s = SettingsStore::read(&p);
        std::fs::write(&p.config, "height_percent=50").unwrap();
        assert!(matches!(
            SettingsStore::write(&p, &s, &[(SettingKey::HeightPercent, SettingValue::U8(60))]),
            Err(StoreError::Conflict(_))
        ));
    }
    #[test]
    fn invalid_utf8_is_unreadable() {
        let p = temp();
        std::fs::create_dir_all(p.config.parent().unwrap()).unwrap();
        std::fs::write(&p.config, [255]).unwrap();
        assert!(matches!(
            SettingsStore::read(&p).status,
            ConfigStatus::Unreadable(_)
        ));
    }
    #[test]
    fn malformed_toml_does_not_report_defaults_as_recovered_fields() {
        let states = fields("height_percent = \n");
        assert!(states
            .iter()
            .all(|field| field.value.is_none() && field.error.is_some()));
    }
    #[test]
    fn failed_create_new_does_not_remove_another_owners_temp_file() {
        let p = temp();
        std::fs::create_dir_all(p.config.parent().unwrap()).unwrap();
        let temp_path = p.config.with_extension("toml.occupied.tmp");
        std::fs::write(&temp_path, "owned by another process").unwrap();
        assert!(write_temp_file(&temp_path, b"ours", None).is_err());
        assert_eq!(
            std::fs::read(&temp_path).unwrap(),
            b"owned by another process"
        );
    }
    #[test]
    fn deleting_expected_canonical_config_does_not_resurrect_stale_text() {
        let p = temp();
        std::fs::create_dir_all(p.config.parent().unwrap()).unwrap();
        std::fs::write(&p.config, "height_percent=65\n").unwrap();
        let expected = SettingsStore::read(&p);
        std::fs::remove_file(&p.config).unwrap();
        assert!(matches!(
            SettingsStore::write(
                &p,
                &expected,
                &[(SettingKey::HeightPercent, SettingValue::U8(70))]
            ),
            Err(StoreError::Conflict(_))
        ));
        assert!(!p.config.exists());
    }
    #[test]
    fn changed_legacy_source_conflicts_on_edited_field_and_preserves_legacy() {
        let p = temp();
        std::fs::create_dir_all(p.legacy_config.parent().unwrap()).unwrap();
        std::fs::write(&p.legacy_config, "height_percent=40\n").unwrap();
        let expected = SettingsStore::read(&p);
        std::fs::write(&p.legacy_config, "height_percent=55\n").unwrap();
        assert!(matches!(
            SettingsStore::write(
                &p,
                &expected,
                &[(SettingKey::HeightPercent, SettingValue::U8(70))]
            ),
            Err(StoreError::Conflict(_))
        ));
        assert!(!p.config.exists());
        assert_eq!(
            std::fs::read_to_string(&p.legacy_config).unwrap(),
            "height_percent=55\n"
        );
    }
    #[test]
    fn legacy_change_before_replace_conflicts_instead_of_copying_stale_snapshot() {
        let p = temp();
        std::fs::create_dir_all(p.legacy_config.parent().unwrap()).unwrap();
        std::fs::write(&p.legacy_config, "height_percent=40\n").unwrap();
        let expected = SettingsStore::read(&p);
        let result = SettingsStore::write_with_hook(
            &p,
            &expected,
            &[(SettingKey::HeightPercent, SettingValue::U8(70))],
            || std::fs::write(&p.legacy_config, "height_percent=60\n").unwrap(),
        );
        assert!(matches!(result, Err(StoreError::Conflict(_))));
        assert!(!p.config.exists());
        assert_eq!(
            std::fs::read_to_string(&p.legacy_config).unwrap(),
            "height_percent=60\n"
        );
    }
    #[test]
    fn canonical_created_before_replace_is_not_overwritten_by_legacy_copy() {
        let p = temp();
        std::fs::create_dir_all(p.legacy_config.parent().unwrap()).unwrap();
        std::fs::write(&p.legacy_config, "height_percent=40\n").unwrap();
        let expected = SettingsStore::read(&p);
        let result = SettingsStore::write_with_hook(
            &p,
            &expected,
            &[(SettingKey::HeightPercent, SettingValue::U8(70))],
            || {
                std::fs::create_dir_all(p.config.parent().unwrap()).unwrap();
                std::fs::write(&p.config, "height_percent=60\n").unwrap();
            },
        );
        assert!(matches!(result, Err(StoreError::Conflict(_))));
        assert_eq!(
            std::fs::read_to_string(&p.config).unwrap(),
            "height_percent=60\n"
        );
        assert_eq!(
            std::fs::read_to_string(&p.legacy_config).unwrap(),
            "height_percent=40\n"
        );
    }
    #[test]
    fn canonical_deleted_before_replace_is_not_replaced_from_stale_snapshot() {
        let p = temp();
        std::fs::create_dir_all(p.config.parent().unwrap()).unwrap();
        std::fs::create_dir_all(p.legacy_config.parent().unwrap()).unwrap();
        std::fs::write(&p.config, "height_percent=50\n").unwrap();
        std::fs::write(&p.legacy_config, "height_percent=40\n").unwrap();
        let expected = SettingsStore::read(&p);
        let result = SettingsStore::write_with_hook(
            &p,
            &expected,
            &[(SettingKey::HeightPercent, SettingValue::U8(70))],
            || std::fs::remove_file(&p.config).unwrap(),
        );
        assert!(matches!(result, Err(StoreError::Conflict(_))));
        assert_eq!(
            std::fs::read_to_string(&p.legacy_config).unwrap(),
            "height_percent=40\n"
        );
        assert!(!p.config.exists());
    }
    #[cfg(unix)]
    #[test]
    fn refuses_canonical_symlink_writes_without_replacing_or_modifying_target() {
        use std::os::unix::fs::symlink;
        let p = temp();
        std::fs::create_dir_all(p.config.parent().unwrap()).unwrap();
        let target = p.config.parent().unwrap().join("real-config.toml");
        std::fs::write(&target, "height_percent=42\n").unwrap();
        symlink(&target, &p.config).unwrap();
        let expected = SettingsStore::read(&p);
        assert_eq!(expected.text, "height_percent=42\n");
        let err = SettingsStore::write(
            &p,
            &expected,
            &[(SettingKey::HeightPercent, SettingValue::U8(70))],
        )
        .unwrap_err();
        assert!(matches!(err, StoreError::Inaccessible(_)));
        assert!(format!("{err:?}").contains("symbolic link"));
        assert!(p
            .config
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "height_percent=42\n"
        );
    }
    #[cfg(unix)]
    #[test]
    fn replacement_preserves_existing_canonical_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let p = temp();
        std::fs::create_dir_all(p.config.parent().unwrap()).unwrap();
        std::fs::write(&p.config, "height_percent=40\n").unwrap();
        std::fs::set_permissions(&p.config, std::fs::Permissions::from_mode(0o600)).unwrap();
        let expected = SettingsStore::read(&p);
        SettingsStore::write(
            &p,
            &expected,
            &[(SettingKey::HeightPercent, SettingValue::U8(70))],
        )
        .unwrap();
        assert_eq!(
            std::fs::metadata(&p.config).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    #[cfg(unix)]
    #[test]
    fn newly_created_canonical_config_uses_private_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let p = temp();
        let expected = SettingsStore::read(&p);
        SettingsStore::write(
            &p,
            &expected,
            &[(SettingKey::HeightPercent, SettingValue::U8(70))],
        )
        .unwrap();
        assert_eq!(
            std::fs::metadata(&p.config).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    #[cfg(unix)]
    #[test]
    fn canonical_symlink_swap_before_replace_is_rejected() {
        use std::os::unix::fs::symlink;
        let p = temp();
        std::fs::create_dir_all(p.config.parent().unwrap()).unwrap();
        std::fs::write(&p.config, "height_percent=40\n").unwrap();
        let target = p.config.parent().unwrap().join("symlink-target.toml");
        std::fs::write(&target, "height_percent=42\n").unwrap();
        let expected = SettingsStore::read(&p);
        let result = SettingsStore::write_with_hook(
            &p,
            &expected,
            &[(SettingKey::HeightPercent, SettingValue::U8(70))],
            || {
                std::fs::remove_file(&p.config).unwrap();
                symlink(&target, &p.config).unwrap();
            },
        );
        assert!(matches!(result, Err(StoreError::Inaccessible(_))));
        assert!(p
            .config
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "height_percent=42\n"
        );
    }
    #[test]
    fn unreadable_snapshot_cannot_be_used_to_overwrite_canonical_config() {
        let p = temp();
        std::fs::create_dir_all(p.config.parent().unwrap()).unwrap();
        std::fs::write(&p.config, [255]).unwrap();
        let expected = SettingsStore::read(&p);
        assert!(matches!(expected.status, ConfigStatus::Unreadable(_)));
        assert!(matches!(
            SettingsStore::write(
                &p,
                &expected,
                &[(SettingKey::HeightPercent, SettingValue::U8(70))]
            ),
            Err(StoreError::Inaccessible(_))
        ));
        assert_eq!(std::fs::read(&p.config).unwrap(), [255]);
    }
    #[test]
    fn detects_unknown_keys_and_field_types_but_uses_full_config_validation() {
        let p = temp();
        std::fs::create_dir_all(p.config.parent().unwrap()).unwrap();
        std::fs::write(
            &p.config,
            "height_percent=101\nwidth_percent='bad'\nextra=1",
        )
        .unwrap();
        let s = SettingsStore::read(&p);
        assert!(matches!(s.status, ConfigStatus::Invalid));
        assert!(s
            .fields
            .iter()
            .find(|f| f.key == SettingKey::HeightPercent)
            .unwrap()
            .error
            .is_some());
        assert!(s
            .fields
            .iter()
            .find(|f| f.key == SettingKey::WidthPercent)
            .unwrap()
            .error
            .is_some());
        assert!(Config::from_toml_str(&s.text).is_err());
    }
    #[test]
    fn supports_all_eleven_keys_and_rejects_bad_ranges() {
        assert_eq!(SettingKey::ALL.len(), 11);
        assert!(decode(SettingKey::HeightPercent, &toml::Value::Integer(101)).is_err());
        assert!(decode(SettingKey::AnimationMs, &toml::Value::Integer(2001)).is_err());
        assert!(decode(
            SettingKey::Hotkey,
            &toml::Value::String("Ctrl+NotAKey".into())
        )
        .is_err());
    }
    #[test]
    fn preserves_unrelated_external_edit_and_does_not_rewrite_malformed_files() {
        let p = temp();
        std::fs::create_dir_all(p.config.parent().unwrap()).unwrap();
        std::fs::write(&p.config, "height_percent=40\nwidth_percent=70\n").unwrap();
        let snapshot = SettingsStore::read(&p);
        std::fs::write(&p.config, "height_percent=40\nwidth_percent=80\n").unwrap();
        let result = SettingsStore::write(
            &p,
            &snapshot,
            &[(SettingKey::HeightPercent, SettingValue::U8(60))],
        )
        .unwrap();
        assert!(result.text.contains("height_percent=60"));
        assert!(result.text.contains("width_percent=80"));
        std::fs::write(&p.config, "height_percent = ").unwrap();
        let malformed = SettingsStore::read(&p);
        assert!(matches!(malformed.status, ConfigStatus::Malformed(_)));
        assert!(matches!(
            SettingsStore::write(
                &p,
                &malformed,
                &[(SettingKey::HeightPercent, SettingValue::U8(65))]
            ),
            Err(StoreError::Malformed(_))
        ));
        assert_eq!(
            std::fs::read_to_string(&p.config).unwrap(),
            "height_percent = "
        );
    }
    #[test]
    fn creates_canonical_from_legacy_without_modifying_legacy() {
        let p = temp();
        std::fs::create_dir_all(p.legacy_config.parent().unwrap()).unwrap();
        let legacy = b"# legacy\nheight_percent=40\n";
        std::fs::write(&p.legacy_config, legacy).unwrap();
        let snapshot = SettingsStore::read(&p);
        assert_eq!(snapshot.path, p.legacy_config);
        let saved = SettingsStore::write(
            &p,
            &snapshot,
            &[(SettingKey::HeightPercent, SettingValue::U8(55))],
        )
        .unwrap();
        assert_eq!(saved.path, p.config);
        assert!(saved.text.contains("height_percent=55"));
        assert_eq!(std::fs::read(&p.legacy_config).unwrap(), legacy);
    }
    fn temp() -> AppPaths {
        let d = std::env::temp_dir().join(format!(
            "regie-settings-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        AppPaths::new(d.as_path())
    }
}
