//! AppKit-owned controls. All quit items are ordinary commands, never direct exits.
use crate::config::Appearance;
use crate::{about_metadata, APP_NAME};
use tray_icon::menu::accelerator::{Accelerator, Code, Modifiers};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

#[derive(Debug, Clone, Copy)]
pub enum AppCommand {
    OpenSettings,
    NewTab,
    CloseActiveTab,
    Quit,
}

pub struct AppUi {
    pub tray: Menu,
    _menubar: Menu,
    tray_settings: MenuItem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAppearance {
    System,
    Light,
    Dark,
}

pub fn apply_appearance_with(appearance: Appearance, apply: impl FnOnce(NativeAppearance)) {
    let native = match appearance {
        Appearance::System => NativeAppearance::System,
        Appearance::Light => NativeAppearance::Light,
        Appearance::Dark => NativeAppearance::Dark,
    };
    apply(native);
}

pub fn apply_appearance(appearance: Appearance) {
    #[cfg(target_os = "macos")]
    set_appearance(appearance);
    #[cfg(not(target_os = "macos"))]
    let _ = appearance;
}

impl AppUi {
    pub fn new(
        on_command: impl Fn(AppCommand) + Send + Sync + 'static,
        appearance: Appearance,
    ) -> Result<Self, String> {
        let menubar = Menu::new();
        let app = Submenu::new(APP_NAME, true);
        let file = Submenu::new("File", true);
        let new = MenuItem::new(
            "New Tab",
            true,
            Some(Accelerator::new(Modifiers::META, Code::KeyT)),
        );
        let close = MenuItem::new(
            "Close Tab",
            true,
            Some(Accelerator::new(Modifiers::META, Code::KeyW)),
        );
        let quit = MenuItem::new(
            format!("Quit {APP_NAME}"),
            true,
            Some(Accelerator::new(Modifiers::META, Code::KeyQ)),
        );
        let tray_quit = MenuItem::new(format!("Quit {APP_NAME}"), true, None);
        let settings = MenuItem::new(
            "Settings…",
            true,
            Some(Accelerator::new(Modifiers::META, Code::Comma)),
        );
        let tray_settings = MenuItem::new("Settings…", true, None);
        let tray = Menu::new();
        let about = PredefinedMenuItem::about(None, Some(about_metadata()));
        app.append_items(&[
            &about,
            &PredefinedMenuItem::separator(),
            &settings,
            &PredefinedMenuItem::separator(),
            &quit,
        ])
        .map_err(|e| e.to_string())?;
        file.append_items(&[&new, &close])
            .map_err(|e| e.to_string())?;
        menubar
            .append_items(&[&app, &file])
            .map_err(|e| e.to_string())?;
        tray.append_items(&[
            &about,
            &tray_settings,
            &PredefinedMenuItem::separator(),
            &tray_quit,
        ])
        .map_err(|e| e.to_string())?;
        let actions = [
            (settings.id().clone(), AppCommand::OpenSettings),
            (tray_settings.id().clone(), AppCommand::OpenSettings),
            (new.id().clone(), AppCommand::NewTab),
            (close.id().clone(), AppCommand::CloseActiveTab),
            (quit.id().clone(), AppCommand::Quit),
            (tray_quit.id().clone(), AppCommand::Quit),
        ];
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if let Some((_, command)) = actions.iter().find(|(id, _)| *id == event.id) {
                on_command(*command);
            }
        }));
        #[cfg(target_os = "macos")]
        {
            menubar.init_for_nsapp();
            set_appearance(appearance);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = appearance;
        Ok(Self {
            tray,
            _menubar: menubar,
            tray_settings,
        })
    }

    pub fn set_config_warning(&self, warning: bool) {
        self.tray_settings.set_text(if warning {
            "Configuration needs attention…"
        } else {
            "Settings…"
        });
    }
}

#[cfg(target_os = "macos")]
pub fn set_appearance(appearance: Appearance) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication,
    };
    let marker = MainThreadMarker::new().expect("AppKit runs on main thread");
    apply_appearance_with(appearance, |appearance| {
        let appearance = match appearance {
            NativeAppearance::System => None,
            NativeAppearance::Light => {
                NSAppearance::appearanceNamed(unsafe { NSAppearanceNameAqua })
            }
            NativeAppearance::Dark => {
                NSAppearance::appearanceNamed(unsafe { NSAppearanceNameDarkAqua })
            }
        };
        NSApplication::sharedApplication(marker).setAppearance(appearance.as_deref());
    });
}

#[cfg(target_os = "macos")]
pub fn confirm_termination(names: &[String], appearance: Appearance) -> bool {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSAlert;
    use objc2_foundation::NSString;
    let marker = MainThreadMarker::new().expect("AppKit runs on main thread");
    set_appearance(appearance);
    let alert = NSAlert::new(marker);
    alert.setMessageText(&NSString::from_str("Terminate running apps?"));
    alert.setInformativeText(&NSString::from_str(&format!(
        "Running apps may lose unsaved work.\n\n{}",
        names.join("\n")
    )));
    let cancel = alert.addButtonWithTitle(&NSString::from_str("Cancel"));
    cancel.setKeyEquivalent(&NSString::from_str("\r"));
    alert.addButtonWithTitle(&NSString::from_str("Terminate"));
    // Regie's panel sits at level 24; an ordinary modal alert would be hidden below it.
    alert.window().setLevel(25);
    alert.runModal() == 1001
}

#[cfg(not(target_os = "macos"))]
pub fn confirm_termination(_: &[String], _: Appearance) -> bool {
    false
}

#[cfg(test)]
mod appearance_tests {
    use super::*;

    #[test]
    fn native_appearance_adapter_maps_each_choice_including_system() {
        let cases = [
            (Appearance::System, NativeAppearance::System),
            (Appearance::Light, NativeAppearance::Light),
            (Appearance::Dark, NativeAppearance::Dark),
        ];
        for (appearance, expected) in cases {
            let mut applied = None;
            apply_appearance_with(appearance, |value| applied = Some(value));
            assert_eq!(applied, Some(expected));
        }
    }
}
