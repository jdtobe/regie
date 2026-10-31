use crate::{
    config::Config,
    settings_store::{ConfigSnapshot, ConfigStatus, SettingKey, SettingValue},
};

#[derive(Debug, Clone)]
pub enum SettingsEffect {
    Write {
        generation: u64,
        changes: Vec<(SettingKey, SettingValue)>,
    },
    Apply {
        generation: u64,
        config: Config,
    },
    ShowWarning(String),
    ClearWarning,
    OpenSettings,
    ShowError(String),
    Render,
}

#[derive(Debug)]
pub enum SettingsResult {
    Write {
        generation: u64,
        result: Result<ConfigSnapshot, String>,
    },
    Apply {
        generation: u64,
        result: Result<(), String>,
    },
}

pub struct SettingsController {
    snapshot: ConfigSnapshot,
    active: Config,
    desired: Config,
    generation: u64,
    pending: Option<(u64, ConfigSnapshot)>,
    warning: Option<String>,
}

impl SettingsController {
    pub fn new(snapshot: ConfigSnapshot, active: Config) -> Self {
        let warning = match &snapshot.status {
            ConfigStatus::Valid(_) => None,
            ConfigStatus::Invalid => Some("Configuration has invalid settings".into()),
            ConfigStatus::Malformed(e) | ConfigStatus::Unreadable(e) => Some(e.clone()),
        };
        Self {
            desired: snapshot_fields_config(&snapshot).unwrap_or_else(|| match &snapshot.status {
                ConfigStatus::Valid(c) => c.clone(),
                _ => active.clone(),
            }),
            snapshot,
            active,
            generation: 0,
            pending: None,
            warning,
        }
    }
    pub fn snapshot(&self) -> &ConfigSnapshot {
        &self.snapshot
    }
    pub fn active(&self) -> &Config {
        &self.active
    }
    pub fn initial_effects(&self) -> Vec<SettingsEffect> {
        match &self.warning {
            Some(message) => vec![
                SettingsEffect::OpenSettings,
                SettingsEffect::ShowWarning(message.clone()),
                SettingsEffect::Render,
            ],
            None => vec![SettingsEffect::Render],
        }
    }

    pub fn edit(&mut self, key: SettingKey, value: SettingValue) -> Vec<SettingsEffect> {
        self.generation += 1;
        let mut proposed = self.desired.clone();
        if !assign(&mut proposed, key, value.clone()) || !valid(&proposed) {
            return vec![
                SettingsEffect::ShowError(format!("Invalid value for {key:?}")),
                SettingsEffect::Render,
            ];
        }
        self.desired = proposed;
        self.pending = Some((self.generation, self.snapshot.clone()));
        vec![
            SettingsEffect::Write {
                generation: self.generation,
                changes: vec![(key, value)],
            },
            SettingsEffect::Render,
        ]
    }

    pub fn reset_all(&mut self) -> Vec<SettingsEffect> {
        self.generation += 1;
        let defaults = Config::default();
        let changes = crate::settings_store::SettingKey::ALL
            .iter()
            .filter_map(|key| value_for(&defaults, *key).map(|v| (*key, v)))
            .collect();
        self.desired = defaults;
        self.pending = Some((self.generation, self.snapshot.clone()));
        vec![
            SettingsEffect::Write {
                generation: self.generation,
                changes,
            },
            SettingsEffect::Render,
        ]
    }

    pub fn external_change(&mut self, snapshot: ConfigSnapshot) -> Vec<SettingsEffect> {
        self.generation += 1;
        self.snapshot = snapshot;
        let mut effects = vec![SettingsEffect::Render];
        if let ConfigStatus::Valid(config) = &self.snapshot.status {
            self.desired = config.clone();
            effects.insert(
                0,
                SettingsEffect::Apply {
                    generation: self.generation,
                    config: config.clone(),
                },
            );
        } else {
            let text = status_error(&self.snapshot.status);
            self.warning = Some(text.clone());
            effects.insert(0, SettingsEffect::ShowWarning(text));
        }
        effects
    }

    pub fn finish(&mut self, result: SettingsResult) -> Vec<SettingsEffect> {
        match result {
            SettingsResult::Write { generation, result } => {
                if self.pending.as_ref().map(|p| p.0) != Some(generation)
                    || generation != self.generation
                {
                    return vec![];
                }
                match result {
                    Err(error) => {
                        self.warning = Some(error.clone());
                        vec![SettingsEffect::ShowError(error), SettingsEffect::Render]
                    }
                    Ok(snapshot) => {
                        self.snapshot = snapshot.clone();
                        match &snapshot.status {
                            ConfigStatus::Valid(config) => {
                                self.desired = config.clone();
                                self.pending = Some((generation, snapshot.clone()));
                                vec![
                                    SettingsEffect::Apply {
                                        generation,
                                        config: config.clone(),
                                    },
                                    SettingsEffect::Render,
                                ]
                            }
                            _ => {
                                let msg = status_error(&snapshot.status);
                                self.warning = Some(msg.clone());
                                self.pending = None;
                                vec![SettingsEffect::ShowWarning(msg), SettingsEffect::Render]
                            }
                        }
                    }
                }
            }
            SettingsResult::Apply { generation, result } => {
                if generation != self.generation {
                    return vec![];
                }
                match result {
                    Ok(()) => {
                        if let ConfigStatus::Valid(config) = &self.snapshot.status {
                            self.active = config.clone();
                        }
                        self.warning = None;
                        self.pending = None;
                        vec![SettingsEffect::ClearWarning, SettingsEffect::Render]
                    }
                    Err(error) => {
                        self.warning = Some(error.clone());
                        vec![SettingsEffect::ShowError(error), SettingsEffect::Render]
                    }
                }
            }
        }
    }
}

fn status_error(status: &ConfigStatus) -> String {
    match status {
        ConfigStatus::Valid(_) => String::new(),
        ConfigStatus::Invalid => "Configuration has invalid settings".into(),
        ConfigStatus::Malformed(e) | ConfigStatus::Unreadable(e) => e.clone(),
    }
}
fn assign(c: &mut Config, k: SettingKey, v: SettingValue) -> bool {
    use SettingKey as K;
    use SettingValue as V;
    match (k, v) {
        (K::Hotkey, V::String(x)) => c.hotkey = x,
        (K::HeightPercent, V::U8(x)) => c.height_percent = x,
        (K::WidthPercent, V::U8(x)) => c.width_percent = x,
        (K::HideOnFocusLost, V::Bool(x)) => c.hide_on_focus_lost = x,
        (K::AnimationMs, V::U32(x)) => c.animation_ms = x,
        (K::TabPosition, V::TabPosition(x)) => c.tab_position = x,
        (K::NewTabDirectory, V::NewTabDirectory(x)) => c.new_tab_directory = x,
        (K::StartupTabs, V::StartupTabs(x)) => c.startup_tabs = x,
        (K::LastTabAction, V::LastTabAction(x)) => c.last_tab_action = x,
        (K::Appearance, V::Appearance(x)) => c.appearance = x,
        (K::TerminalTheme, V::TerminalTheme(x)) => c.terminal_theme = x,
        _ => return false,
    }
    true
}
fn value_for(c: &Config, k: SettingKey) -> Option<SettingValue> {
    use SettingKey::*;
    Some(match k {
        Hotkey => SettingValue::String(c.hotkey.clone()),
        HeightPercent => SettingValue::U8(c.height_percent),
        WidthPercent => SettingValue::U8(c.width_percent),
        HideOnFocusLost => SettingValue::Bool(c.hide_on_focus_lost),
        AnimationMs => SettingValue::U32(c.animation_ms),
        TabPosition => SettingValue::TabPosition(c.tab_position),
        NewTabDirectory => SettingValue::NewTabDirectory(c.new_tab_directory),
        StartupTabs => SettingValue::StartupTabs(c.startup_tabs),
        LastTabAction => SettingValue::LastTabAction(c.last_tab_action),
        Appearance => SettingValue::Appearance(c.appearance),
        TerminalTheme => SettingValue::TerminalTheme(c.terminal_theme),
    })
}
fn valid(c: &Config) -> bool {
    c.hotkey().is_ok()
        && crate::config::HEIGHT_PERCENT_RANGE.contains(&c.height_percent)
        && crate::config::WIDTH_PERCENT_RANGE.contains(&c.width_percent)
        && c.animation_ms <= crate::config::ANIMATION_MS_MAX
}
fn snapshot_fields_config(snapshot: &ConfigSnapshot) -> Option<Config> {
    if snapshot.fields.len() != SettingKey::ALL.len() {
        return None;
    }
    let mut config = Config::default();
    for field in &snapshot.fields {
        if let Some(value) = field.value.clone() {
            if !assign(&mut config, field.key, value) {
                return None;
            }
        }
    }
    Some(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    fn snap(c: Config) -> ConfigSnapshot {
        ConfigSnapshot {
            path: PathBuf::new(),
            text: String::new(),
            fields: vec![],
            status: ConfigStatus::Valid(c),
        }
    }
    fn invalid_snapshot(status: ConfigStatus) -> ConfigSnapshot {
        ConfigSnapshot {
            path: PathBuf::new(),
            text: String::new(),
            fields: SettingKey::ALL
                .iter()
                .map(|key| crate::settings_store::FieldState {
                    key: *key,
                    value: None,
                    error: Some("invalid field".into()),
                })
                .collect(),
            status,
        }
    }
    #[test]
    fn invalid_startup_requests_settings_and_shows_warning() {
        let c = Config::default();
        let s = SettingsController::new(
            invalid_snapshot(ConfigStatus::Malformed("bad TOML".into())),
            c,
        );
        let effects = s.initial_effects();
        assert!(effects
            .iter()
            .any(|e| matches!(e, SettingsEffect::OpenSettings)));
        assert!(effects
            .iter()
            .any(|e| matches!(e, SettingsEffect::ShowWarning(_))));
    }
    #[test]
    fn startup_malformed_and_unreadable_both_request_settings() {
        for status in [
            ConfigStatus::Malformed("parse failure".into()),
            ConfigStatus::Unreadable("read failure".into()),
        ] {
            let s = SettingsController::new(invalid_snapshot(status), Config::default());
            assert!(s
                .initial_effects()
                .iter()
                .any(|e| matches!(e, SettingsEffect::OpenSettings)));
        }
    }
    #[test]
    fn valid_edit_can_persist_while_unknown_key_keeps_document_inactive() {
        let mut snapshot = invalid_snapshot(ConfigStatus::Invalid);
        for f in &mut snapshot.fields {
            f.value = Some(value_for(&Config::default(), f.key).unwrap());
            f.error = None;
        }
        snapshot
            .fields
            .iter_mut()
            .find(|f| f.key == SettingKey::HeightPercent)
            .unwrap()
            .value = None;
        snapshot
            .fields
            .iter_mut()
            .find(|f| f.key == SettingKey::HeightPercent)
            .unwrap()
            .error = Some("invalid height".into());
        let mut s = SettingsController::new(snapshot, Config::default());
        assert!(matches!(
            s.edit(SettingKey::WidthPercent, SettingValue::U8(75))[0],
            SettingsEffect::Write { .. }
        ));
        let g = s.generation;
        let mut still_invalid = invalid_snapshot(ConfigStatus::Invalid);
        for f in &mut still_invalid.fields {
            f.value = Some(value_for(&Config::default(), f.key).unwrap());
            f.error = None;
        }
        assert!(matches!(
            s.finish(SettingsResult::Write {
                generation: g,
                result: Ok(still_invalid)
            })[0],
            SettingsEffect::ShowWarning(_)
        ));
        assert_eq!(s.active().width_percent, 100);
    }
    #[test]
    fn external_invalid_change_keeps_active_then_repair_applies_and_clears_warning() {
        let mut s = SettingsController::new(snap(Config::default()), Config::default());
        let mut bad = invalid_snapshot(ConfigStatus::Invalid);
        for f in &mut bad.fields {
            f.value = Some(value_for(&Config::default(), f.key).unwrap());
            f.error = None;
        }
        bad.fields
            .iter_mut()
            .find(|f| f.key == SettingKey::HeightPercent)
            .unwrap()
            .value = None;
        let effects = s.external_change(bad);
        assert!(matches!(effects[0], SettingsEffect::ShowWarning(_)));
        assert_eq!(s.active().height_percent, 40);
        let write = s.edit(SettingKey::HeightPercent, SettingValue::U8(65));
        let g = match write[0] {
            SettingsEffect::Write { generation, .. } => generation,
            _ => panic!(),
        };
        let done = s.finish(SettingsResult::Write {
            generation: g,
            result: Ok(snap(Config {
                height_percent: 65,
                ..Config::default()
            })),
        });
        assert!(matches!(done[0], SettingsEffect::Apply { .. }));
        let cleared = s.finish(SettingsResult::Apply {
            generation: g,
            result: Ok(()),
        });
        assert!(matches!(cleared[0], SettingsEffect::ClearWarning));
        assert_eq!(s.active().height_percent, 65);
    }
    #[test]
    fn interrupted_reset_and_stale_apply_cannot_clear_new_warning() {
        let c = Config::default();
        let mut s = SettingsController::new(snap(c.clone()), c);
        let reset = s.reset_all();
        let g = match reset[0] {
            SettingsEffect::Write { generation, .. } => generation,
            _ => panic!(),
        };
        let next = s.external_change(invalid_snapshot(ConfigStatus::Malformed("broken".into())));
        assert!(matches!(next[0], SettingsEffect::ShowWarning(_)));
        assert!(s
            .finish(SettingsResult::Write {
                generation: g,
                result: Ok(snap(Config::default()))
            })
            .is_empty());
        assert!(s
            .finish(SettingsResult::Apply {
                generation: g,
                result: Ok(())
            })
            .is_empty());
        assert!(matches!(s.snapshot().status, ConfigStatus::Malformed(_)));
    }
    #[test]
    fn save_status_can_advance_without_advancing_active_until_apply() {
        let mut s = SettingsController::new(snap(Config::default()), Config::default());
        let write = s.edit(SettingKey::HeightPercent, SettingValue::U8(55));
        let g = match write[0] {
            SettingsEffect::Write { generation, .. } => generation,
            _ => panic!(),
        };
        s.finish(SettingsResult::Write {
            generation: g,
            result: Ok(snap(Config {
                height_percent: 55,
                ..Config::default()
            })),
        });
        assert!(matches!(s.snapshot().status, ConfigStatus::Valid(_)));
        assert_eq!(s.active().height_percent, 40);
    }
    #[test]
    fn write_failure_is_visible_and_does_not_claim_saved_value() {
        let mut s = SettingsController::new(snap(Config::default()), Config::default());
        let e = s.edit(SettingKey::HeightPercent, SettingValue::U8(60));
        let g = match e[0] {
            SettingsEffect::Write { generation, .. } => generation,
            _ => panic!(),
        };
        let out = s.finish(SettingsResult::Write {
            generation: g,
            result: Err("disk full".into()),
        });
        assert!(matches!(out[0], SettingsEffect::ShowError(_)));
        assert_eq!(s.active().height_percent, 40);
        assert_eq!(s.snapshot().status, ConfigStatus::Valid(Config::default()));
    }
    #[test]
    fn external_valid_change_is_applied_after_successful_runtime_result() {
        let mut s = SettingsController::new(snap(Config::default()), Config::default());
        let external = Config {
            height_percent: 62,
            ..Config::default()
        };
        let effects = s.external_change(snap(external.clone()));
        let g = match effects[0] {
            SettingsEffect::Apply { generation, .. } => generation,
            _ => panic!(),
        };
        assert_eq!(s.active().height_percent, 40);
        s.finish(SettingsResult::Apply {
            generation: g,
            result: Ok(()),
        });
        assert_eq!(s.active(), &external);
    }
    #[test]
    fn edit_to_default_resets_single_setting() {
        let mut s = SettingsController::new(
            snap(Config {
                height_percent: 70,
                ..Config::default()
            }),
            Config {
                height_percent: 70,
                ..Config::default()
            },
        );
        let e = s.edit(
            SettingKey::HeightPercent,
            SettingValue::U8(Config::default().height_percent),
        );
        match &e[0] {
            SettingsEffect::Write { changes, .. } => assert_eq!(
                changes,
                &vec![(SettingKey::HeightPercent, SettingValue::U8(40))]
            ),
            _ => panic!(),
        }
    }
    #[test]
    fn edit_emits_write_and_applies_only_after_success() {
        let c = Config::default();
        let mut s = SettingsController::new(snap(c.clone()), c);
        let e = s.edit(SettingKey::HeightPercent, SettingValue::U8(60));
        let g = match e[0] {
            SettingsEffect::Write { generation, .. } => generation,
            _ => panic!(),
        };
        assert_eq!(s.active().height_percent, 40);
        let effects = s.finish(SettingsResult::Write {
            generation: g,
            result: Ok(snap(Config {
                height_percent: 60,
                ..Config::default()
            })),
        });
        assert!(matches!(effects[0], SettingsEffect::Apply { .. }));
    }
    #[test]
    fn stale_write_completion_cannot_replace_newer_snapshot() {
        let c = Config::default();
        let mut s = SettingsController::new(snap(c.clone()), c);
        let first = s.edit(SettingKey::HeightPercent, SettingValue::U8(60));
        let g = match first[0] {
            SettingsEffect::Write { generation, .. } => generation,
            _ => panic!(),
        };
        s.edit(SettingKey::WidthPercent, SettingValue::U8(70));
        assert!(s
            .finish(SettingsResult::Write {
                generation: g,
                result: Err("old failure".into())
            })
            .is_empty());
    }
    #[test]
    fn reset_all_writes_only_config_setting_keys() {
        let c = Config::default();
        let mut s = SettingsController::new(snap(c.clone()), c);
        let effects = s.reset_all();
        match &effects[0] {
            SettingsEffect::Write { changes, .. } => assert_eq!(changes.len(), 11),
            _ => panic!(),
        }
    }
    #[test]
    fn failed_apply_keeps_last_active_config_and_reports_error() {
        let c = Config::default();
        let mut s = SettingsController::new(snap(c.clone()), c);
        let effects = s.external_change(snap(Config {
            height_percent: 60,
            ..Config::default()
        }));
        let g = match effects[0] {
            SettingsEffect::Apply { generation, .. } => generation,
            _ => panic!(),
        };
        let out = s.finish(SettingsResult::Apply {
            generation: g,
            result: Err("runtime failure".into()),
        });
        assert!(matches!(out[0], SettingsEffect::ShowError(_)));
        assert_eq!(s.active().height_percent, 40);
    }
}
