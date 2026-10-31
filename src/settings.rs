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
            desired: match &snapshot.status {
                ConfigStatus::Valid(c) => c.clone(),
                _ => active.clone(),
            },
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
