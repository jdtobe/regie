//! Global hotkey parsing and the show/hide toggle state.

use global_hotkey::hotkey::HotKey;
use global_hotkey::HotKeyState;

pub const DEFAULT_HOTKEY: &str = "Ctrl+`";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Hidden,
    Shown,
}

impl Visibility {
    pub fn toggled(self) -> Self {
        match self {
            Visibility::Hidden => Visibility::Shown,
            Visibility::Shown => Visibility::Hidden,
        }
    }

    /// What to do when the window gains or loses focus.
    pub fn after_focus_change(self, hide_on_focus_lost: bool, focused: bool) -> Self {
        if hide_on_focus_lost && !focused {
            Visibility::Hidden
        } else {
            self
        }
    }
}

pub fn parse_hotkey(text: &str) -> Result<HotKey, String> {
    text.parse()
        .map_err(|e| format!("invalid hotkey {text:?}: {e}"))
}

pub trait HotkeyRegistry {
    fn register(&mut self, key: HotKey) -> Result<(), String>;
    fn unregister(&mut self, key: HotKey) -> Result<(), String>;
}

#[derive(Debug, PartialEq, Eq)]
pub struct HotkeyReplaceError(String);

impl std::fmt::Display for HotkeyReplaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for HotkeyReplaceError {}

/// Registers the replacement before dropping the old key, then persists. A failed save restores
/// the previous registration and removes the replacement; rollback failures are explicit.
pub fn replace_hotkey<R: HotkeyRegistry>(
    manager: &mut R,
    old: HotKey,
    new: HotKey,
    persist: impl FnOnce() -> Result<(), String>,
) -> Result<(), HotkeyReplaceError> {
    if old == new {
        return persist().map_err(HotkeyReplaceError);
    }
    manager.register(new).map_err(|error| {
        HotkeyReplaceError(format!("could not register replacement shortcut: {error}"))
    })?;

    if let Err(error) = manager.unregister(old) {
        let remove_new = manager.unregister(new);
        let restore_old = manager.register(old);
        return Err(HotkeyReplaceError(format!(
            "could not unregister old shortcut: {error}; replacement cleanup: {}; old shortcut restoration: {}",
            describe_result(remove_new), describe_result(restore_old)
        )));
    }

    if let Err(error) = persist() {
        let restore_old = manager.register(old);
        let remove_new = manager.unregister(new);
        return Err(HotkeyReplaceError(format!(
            "could not save shortcut change: {error}; old shortcut restoration: {}; replacement cleanup: {}",
            describe_result(restore_old), describe_result(remove_new)
        )));
    }
    Ok(())
}

fn describe_result(result: Result<(), String>) -> String {
    match result {
        Ok(()) => "succeeded".into(),
        Err(error) => format!("FAILED ({error})"),
    }
}

impl HotkeyRegistry for global_hotkey::GlobalHotKeyManager {
    fn register(&mut self, key: HotKey) -> Result<(), String> {
        global_hotkey::GlobalHotKeyManager::register(self, key).map_err(|error| error.to_string())
    }
    fn unregister(&mut self, key: HotKey) -> Result<(), String> {
        global_hotkey::GlobalHotKeyManager::unregister(self, key).map_err(|error| error.to_string())
    }
}

/// Only a key press toggles; the matching release must be ignored.
pub fn should_toggle(state: HotKeyState) -> bool {
    state == HotKeyState::Pressed
}

#[cfg(test)]
mod tests {
    use super::*;
    use global_hotkey::hotkey::{Code, Modifiers};

    struct FakeRegistry {
        registered: Vec<HotKey>,
        fail_register: bool,
        fail_unregister: bool,
        register_calls: usize,
        fail_register_call: Option<usize>,
    }

    impl FakeRegistry {
        fn new(key: HotKey) -> Self {
            Self {
                registered: vec![key],
                fail_register: false,
                fail_unregister: false,
                register_calls: 0,
                fail_register_call: None,
            }
        }
    }

    impl HotkeyRegistry for FakeRegistry {
        fn register(&mut self, key: HotKey) -> Result<(), String> {
            self.register_calls += 1;
            if self.fail_register || self.fail_register_call == Some(self.register_calls) {
                return Err("registration rejected".into());
            }
            self.registered.push(key);
            Ok(())
        }
        fn unregister(&mut self, key: HotKey) -> Result<(), String> {
            if self.fail_unregister {
                return Err("unregistration failed".into());
            }
            self.registered.retain(|registered| *registered != key);
            Ok(())
        }
    }

    #[test]
    fn rejected_replacement_leaves_the_old_shortcut_registered_and_does_not_save() {
        let old = parse_hotkey(DEFAULT_HOTKEY).unwrap();
        let new = parse_hotkey("Alt+Space").unwrap();
        let mut registry = FakeRegistry::new(old);
        registry.fail_register = true;
        let mut saved = false;
        let error = replace_hotkey(&mut registry, old, new, || {
            saved = true;
            Ok(())
        })
        .unwrap_err();
        assert!(error.to_string().contains("registration rejected"));
        assert_eq!(registry.registered, vec![old]);
        assert!(!saved);
    }

    #[test]
    fn unchanged_shortcut_saves_without_unregistering_or_reregistering() {
        let old = parse_hotkey(DEFAULT_HOTKEY).unwrap();
        let mut registry = FakeRegistry::new(old);
        replace_hotkey(&mut registry, old, old, || Ok(())).unwrap();
        assert_eq!(registry.registered, vec![old]);
    }

    #[test]
    fn failed_save_restores_old_shortcut_and_removes_replacement() {
        let old = parse_hotkey(DEFAULT_HOTKEY).unwrap();
        let new = parse_hotkey("Alt+Space").unwrap();
        let mut registry = FakeRegistry::new(old);
        let error =
            replace_hotkey(&mut registry, old, new, || Err("disk full".into())).unwrap_err();
        assert!(error.to_string().contains("disk full"));
        assert_eq!(registry.registered, vec![old]);
    }

    #[test]
    fn failed_restore_is_reported_without_claiming_old_shortcut_is_registered() {
        let old = parse_hotkey(DEFAULT_HOTKEY).unwrap();
        let new = parse_hotkey("Alt+Space").unwrap();
        let mut registry = FakeRegistry::new(old);
        registry.fail_register_call = Some(2);
        let error = replace_hotkey(&mut registry, old, new, || Err("disk full".into()))
            .unwrap_err()
            .to_string();
        assert!(error.contains("old shortcut restoration: FAILED"));
        assert!(!registry.registered.contains(&old));
    }

    #[test]
    fn default_hotkey_parses_to_ctrl_backquote() {
        let key = parse_hotkey(DEFAULT_HOTKEY).unwrap();
        assert_eq!(key, HotKey::new(Some(Modifiers::CONTROL), Code::Backquote));
    }

    #[test]
    fn parses_other_combinations() {
        let key = parse_hotkey("Alt+Space").unwrap();
        assert_eq!(key, HotKey::new(Some(Modifiers::ALT), Code::Space));
    }

    #[test]
    fn rejects_garbage_and_names_the_input() {
        let err = parse_hotkey("Ctrl+NotAKey").unwrap_err();
        assert!(err.contains("Ctrl+NotAKey"), "error was: {err}");
    }

    #[test]
    fn rejects_modifier_only_input() {
        assert!(parse_hotkey("Ctrl").is_err());
    }

    #[test]
    fn visibility_toggles_both_ways() {
        assert_eq!(Visibility::Hidden.toggled(), Visibility::Shown);
        assert_eq!(Visibility::Shown.toggled(), Visibility::Hidden);
    }

    #[test]
    fn losing_focus_hides_only_when_enabled() {
        assert_eq!(
            Visibility::Shown.after_focus_change(true, false),
            Visibility::Hidden
        );
        assert_eq!(
            Visibility::Shown.after_focus_change(false, false),
            Visibility::Shown
        );
    }

    #[test]
    fn gaining_focus_never_changes_visibility() {
        for hide in [true, false] {
            assert_eq!(
                Visibility::Shown.after_focus_change(hide, true),
                Visibility::Shown
            );
            assert_eq!(
                Visibility::Hidden.after_focus_change(hide, true),
                Visibility::Hidden
            );
        }
    }

    #[test]
    fn losing_focus_while_hidden_stays_hidden() {
        assert_eq!(
            Visibility::Hidden.after_focus_change(true, false),
            Visibility::Hidden
        );
    }

    #[test]
    fn only_press_toggles() {
        assert!(should_toggle(HotKeyState::Pressed));
        assert!(!should_toggle(HotKeyState::Released));
    }
}
