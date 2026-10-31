use crate::{
    config::Config,
    settings_store::{ConfigSnapshot, ConfigStatus, SettingKey, SettingValue},
};

#[derive(Debug)]
enum Operation {
    Writing { generation: u64 },
    Applying { generation: u64, config: Config },
}

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
    operation: Option<Operation>,
    queued_changes: Vec<(SettingKey, SettingValue)>,
    inflight_changes: Vec<(SettingKey, SettingValue)>,
    deferred_external: Option<ConfigSnapshot>,
    warning: Option<String>,
}

impl SettingsController {
    /// `active` must be the configuration actually running in the app. In particular, startup
    /// recovery wiring must pass `Config::default()` when the disk snapshot is invalid, because
    /// those safe defaults are what the runtime was started with.
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
            operation: None,
            queued_changes: vec![],
            inflight_changes: vec![],
            deferred_external: None,
            warning,
        }
    }
    pub fn snapshot(&self) -> &ConfigSnapshot {
        &self.snapshot
    }
    pub fn active(&self) -> &Config {
        &self.active
    }
    pub fn draft(&self) -> &Config {
        &self.desired
    }

    /// Retry explicitly retained edits after a failed write.
    pub fn retry(&mut self) -> Vec<SettingsEffect> {
        let mut effects = self.dispatch();
        effects.push(SettingsEffect::Render);
        effects
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
        let mut proposed = self.desired.clone();
        if !assign(&mut proposed, key, value.clone()) || !valid(&proposed) {
            return vec![
                SettingsEffect::ShowError(format!("Invalid value for {key:?}")),
                SettingsEffect::Render,
            ];
        }
        self.desired = proposed;
        queue_change(&mut self.queued_changes, key, value);
        let mut effects = self.dispatch();
        effects.push(SettingsEffect::Render);
        effects
    }

    pub fn reset_all(&mut self) -> Vec<SettingsEffect> {
        let defaults = Config::default();
        let changes = crate::settings_store::SettingKey::ALL
            .iter()
            .filter_map(|key| value_for(&defaults, *key).map(|v| (*key, v)))
            .collect();
        self.desired = defaults;
        self.queued_changes = changes;
        let mut effects = self.dispatch();
        effects.push(SettingsEffect::Render);
        effects
    }

    pub fn external_change(&mut self, snapshot: ConfigSnapshot) -> Vec<SettingsEffect> {
        if self.operation.is_some() {
            if !matches!(snapshot.status, ConfigStatus::Valid(_)) {
                let warning = status_error(&snapshot.status);
                self.warning = Some(warning.clone());
                self.deferred_external = Some(snapshot);
                return vec![SettingsEffect::ShowWarning(warning), SettingsEffect::Render];
            }
            self.deferred_external = Some(snapshot);
            return vec![SettingsEffect::Render];
        }
        self.generation += 1;
        self.snapshot = snapshot;
        let mut effects = vec![SettingsEffect::Render];
        if let ConfigStatus::Valid(config) = &self.snapshot.status {
            self.desired = config.clone();
            self.operation = Some(Operation::Applying {
                generation: self.generation,
                config: config.clone(),
            });
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
                if !matches!(self.operation,Some(Operation::Writing{generation: active}) if active==generation)
                {
                    return vec![];
                }
                self.operation = None;
                if let Some(external) = self.deferred_external.take() {
                    if result.is_err() {
                        retain_inflight_without_replacing_newer(
                            &mut self.queued_changes,
                            std::mem::take(&mut self.inflight_changes),
                        );
                    } else {
                        self.inflight_changes.clear();
                    }
                    self.deferred_external = Some(external);
                    let mut effects = self.dispatch();
                    effects.push(SettingsEffect::Render);
                    return effects;
                }
                match result {
                    Err(error) => {
                        retain_inflight_without_replacing_newer(
                            &mut self.queued_changes,
                            std::mem::take(&mut self.inflight_changes),
                        );
                        self.warning = Some(error.clone());
                        vec![SettingsEffect::ShowError(error), SettingsEffect::Render]
                    }
                    Ok(snapshot) => {
                        self.inflight_changes.clear();
                        self.snapshot = snapshot.clone();
                        match &snapshot.status {
                            ConfigStatus::Valid(config) => {
                                self.operation = Some(Operation::Applying {
                                    generation,
                                    config: config.clone(),
                                });
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
                                let mut effects = vec![SettingsEffect::ShowWarning(msg)];
                                effects.extend(self.dispatch());
                                effects.push(SettingsEffect::Render);
                                effects
                            }
                        }
                    }
                }
            }
            SettingsResult::Apply { generation, result } => {
                let Some(Operation::Applying {
                    generation: active,
                    config,
                }) = self.operation.take()
                else {
                    return vec![];
                };
                if generation != active {
                    self.operation = Some(Operation::Applying {
                        generation: active,
                        config,
                    });
                    return vec![];
                }
                match result {
                    Ok(()) => {
                        self.active = config;
                        self.warning = None;
                        let mut effects = vec![SettingsEffect::ClearWarning];
                        effects.extend(self.dispatch());
                        effects.push(SettingsEffect::Render);
                        effects
                    }
                    Err(error) => {
                        self.warning = Some(error.clone());
                        let mut effects = vec![SettingsEffect::ShowError(error)];
                        effects.extend(self.dispatch());
                        effects.push(SettingsEffect::Render);
                        effects
                    }
                }
            }
        }
    }

    fn dispatch(&mut self) -> Vec<SettingsEffect> {
        if self.operation.is_some() {
            return vec![];
        }
        if let Some(snapshot) = self.deferred_external.take() {
            let previous = self.snapshot.clone();
            self.snapshot = snapshot;
            if let ConfigStatus::Valid(config) = &self.snapshot.status {
                let config = config.clone();
                let mut conflicts = Vec::new();
                self.queued_changes.retain(|(key, _)| {
                    let before = snapshot_value(&previous, *key);
                    let external = value_for(&config, *key);
                    let conflict = before.is_some() && before != external;
                    if conflict {
                        conflicts.push(*key);
                    }
                    !conflict
                });
                self.desired = config.clone();
                for (key, value) in &self.queued_changes {
                    assign(&mut self.desired, *key, value.clone());
                }
                self.generation += 1;
                self.operation = Some(Operation::Applying {
                    generation: self.generation,
                    config,
                });
                let mut effects = if conflicts.is_empty() {
                    vec![]
                } else {
                    vec![SettingsEffect::ShowError(format!(
                        "External settings conflict on {conflicts:?}; review and edit again."
                    ))]
                };
                effects.push(SettingsEffect::Apply {
                    generation: self.generation,
                    config: match &self.operation {
                        Some(Operation::Applying { config, .. }) => config.clone(),
                        _ => unreachable!(),
                    },
                });
                return effects;
            }
            let warning = status_error(&self.snapshot.status);
            self.warning = Some(warning.clone());
            self.queued_changes.clear();
            self.inflight_changes.clear();
            self.desired =
                snapshot_fields_config(&self.snapshot).unwrap_or_else(|| self.active.clone());
            return vec![SettingsEffect::ShowWarning(warning)];
        }
        if self.queued_changes.is_empty() {
            return vec![];
        }
        self.generation += 1;
        self.inflight_changes = std::mem::take(&mut self.queued_changes);
        self.operation = Some(Operation::Writing {
            generation: self.generation,
        });
        vec![SettingsEffect::Write {
            generation: self.generation,
            changes: self.inflight_changes.clone(),
        }]
    }
}

fn queue_change(
    changes: &mut Vec<(SettingKey, SettingValue)>,
    key: SettingKey,
    value: SettingValue,
) {
    if let Some(found) = changes.iter_mut().find(|(existing, _)| *existing == key) {
        found.1 = value;
    } else {
        changes.push((key, value));
    }
}

fn retain_inflight_without_replacing_newer(
    queued: &mut Vec<(SettingKey, SettingValue)>,
    inflight: Vec<(SettingKey, SettingValue)>,
) {
    for (key, value) in inflight {
        if !queued.iter().any(|(queued_key, _)| *queued_key == key) {
            queued.push((key, value));
        }
    }
}

fn snapshot_value(snapshot: &ConfigSnapshot, key: SettingKey) -> Option<SettingValue> {
    snapshot
        .fields
        .iter()
        .find(|field| field.key == key)
        .and_then(|field| field.value.clone())
        .or_else(|| match &snapshot.status {
            ConfigStatus::Valid(config) => value_for(config, key),
            _ => None,
        })
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
        let after_write = s.finish(SettingsResult::Write {
            generation: g,
            result: Ok(snap(Config::default())),
        });
        assert!(after_write
            .iter()
            .all(|effect| !matches!(effect, SettingsEffect::Apply { .. })));
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
                generation: g + 100,
                result: Err("out of order".into())
            })
            .is_empty());
        assert!(matches!(s.operation,Some(Operation::Writing{generation}) if generation==g));
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
    #[test]
    fn edits_during_write_are_coalesced_and_dispatched_only_after_apply() {
        let mut s = SettingsController::new(snap(Config::default()), Config::default());
        let first = s.edit(SettingKey::HeightPercent, SettingValue::U8(55));
        let g1 = match first[0] {
            SettingsEffect::Write { generation, .. } => generation,
            _ => panic!(),
        };
        let second = s.edit(SettingKey::WidthPercent, SettingValue::U8(75));
        assert!(!second
            .iter()
            .any(|e| matches!(e, SettingsEffect::Write { .. })));
        let apply = s.finish(SettingsResult::Write {
            generation: g1,
            result: Ok(snap(Config {
                height_percent: 55,
                ..Config::default()
            })),
        });
        let ga = match apply[0] {
            SettingsEffect::Apply { generation, .. } => generation,
            _ => panic!(),
        };
        let next = s.finish(SettingsResult::Apply {
            generation: ga,
            result: Ok(()),
        });
        assert!(next.iter().any(|e|matches!(e,SettingsEffect::Write{changes,..} if changes.contains(&(SettingKey::WidthPercent,SettingValue::U8(75))))));
    }
    #[test]
    fn invalid_startup_keeps_constructor_active_as_actual_runtime_state() {
        let supplied = Config {
            height_percent: 77,
            ..Config::default()
        };
        let s = SettingsController::new(
            invalid_snapshot(ConfigStatus::Malformed("bad".into())),
            supplied.clone(),
        );
        assert_eq!(s.active(), &supplied);
    }
    #[test]
    fn failed_write_reports_and_retains_draft_for_later_edit() {
        let mut s = SettingsController::new(snap(Config::default()), Config::default());
        let first = s.edit(SettingKey::HeightPercent, SettingValue::U8(55));
        let g = match first[0] {
            SettingsEffect::Write { generation, .. } => generation,
            _ => panic!(),
        };
        let failed = s.finish(SettingsResult::Write {
            generation: g,
            result: Err("disk full".into()),
        });
        assert!(failed
            .iter()
            .any(|e| matches!(e, SettingsEffect::ShowError(_))));
        assert_eq!(s.draft().height_percent, 55);
        let next = s.edit(SettingKey::WidthPercent, SettingValue::U8(75));
        assert!(next.iter().any(|e|matches!(e,SettingsEffect::Write{changes,..} if changes.contains(&(SettingKey::HeightPercent,SettingValue::U8(55))))));
    }
    #[test]
    fn newer_external_change_waits_for_prior_apply_and_wins_without_regression() {
        let mut s = SettingsController::new(snap(Config::default()), Config::default());
        let first = Config {
            height_percent: 55,
            ..Config::default()
        };
        let first_effect = s.external_change(snap(first.clone()));
        let g1 = match first_effect[0] {
            SettingsEffect::Apply { generation, .. } => generation,
            _ => panic!(),
        };
        let newer = Config {
            height_percent: 65,
            ..Config::default()
        };
        assert!(s
            .external_change(snap(newer.clone()))
            .iter()
            .all(|e| !matches!(e, SettingsEffect::Apply { .. })));
        let next = s.finish(SettingsResult::Apply {
            generation: g1,
            result: Ok(()),
        });
        assert_eq!(s.active(), &first);
        let g2 = next
            .iter()
            .find_map(|e| match e {
                SettingsEffect::Apply { generation, .. } => Some(*generation),
                _ => None,
            })
            .unwrap();
        s.finish(SettingsResult::Apply {
            generation: g2,
            result: Ok(()),
        });
        assert_eq!(s.active(), &newer);
    }
    #[test]
    fn failed_write_requires_explicit_retry() {
        let mut s = SettingsController::new(snap(Config::default()), Config::default());
        let e = s.edit(SettingKey::HeightPercent, SettingValue::U8(55));
        let g = match e[0] {
            SettingsEffect::Write { generation, .. } => generation,
            _ => panic!(),
        };
        s.finish(SettingsResult::Write {
            generation: g,
            result: Err("offline".into()),
        });
        assert!(s.retry().iter().any(|e|matches!(e,SettingsEffect::Write{changes,..} if changes.contains(&(SettingKey::HeightPercent,SettingValue::U8(55))))));
    }
    #[test]
    fn external_snapshot_reconciles_queued_changes_and_reports_same_field_conflict() {
        let mut s = SettingsController::new(snap(Config::default()), Config::default());
        let first = s.edit(SettingKey::WidthPercent, SettingValue::U8(70));
        let g = match first[0] {
            SettingsEffect::Write { generation, .. } => generation,
            _ => panic!(),
        };
        s.edit(SettingKey::HeightPercent, SettingValue::U8(55));
        let ext = Config {
            height_percent: 65,
            width_percent: 80,
            ..Config::default()
        };
        s.external_change(snap(ext.clone()));
        let resolved = s.finish(SettingsResult::Write {
            generation: g,
            result: Ok(snap(Config {
                width_percent: 70,
                ..Config::default()
            })),
        });
        assert!(resolved
            .iter()
            .any(|e| matches!(e,SettingsEffect::ShowError(msg) if msg.contains("HeightPercent"))));
        assert_eq!(s.draft(), &ext);
        assert!(s.queued_changes.is_empty());
    }
    #[test]
    fn old_failed_same_key_write_does_not_replace_newer_queued_edit() {
        let mut s = SettingsController::new(snap(Config::default()), Config::default());
        let first = s.edit(SettingKey::HeightPercent, SettingValue::U8(55));
        let g = match first[0] {
            SettingsEffect::Write { generation, .. } => generation,
            _ => panic!(),
        };
        s.edit(SettingKey::HeightPercent, SettingValue::U8(65));
        s.finish(SettingsResult::Write {
            generation: g,
            result: Err("offline".into()),
        });
        let retry = s.retry();
        assert!(retry.iter().any(|e|matches!(e,SettingsEffect::Write{changes,..} if changes.contains(&(SettingKey::HeightPercent,SettingValue::U8(65))))));
        assert!(!retry.iter().any(|e|matches!(e,SettingsEffect::Write{changes,..} if changes.contains(&(SettingKey::HeightPercent,SettingValue::U8(55))))));
    }
}
