//! Main-thread OS wiring; session and tab policy live in Workspace.
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use regie::app_paths::AppPaths;
use regie::app_ui::{self, AppCommand, AppUi};
use regie::clipboard;
use regie::config;
use regie::dropdown::DropDown;
use regie::hotkey::{should_toggle, Visibility};
use regie::session_output::SessionOutput;
use regie::settings::{SettingsController, SettingsEffect, SettingsResult};
use regie::settings_store::{ConfigStatus, SettingKey, SettingsStore};
use regie::settings_view::{self, SettingsRequest, SettingsView};
use regie::settings_watch::{SettingsWatch, SettingsWatchEvent};
use regie::terminal_protocol::{parse_workspace_request, UiDelivery};
use regie::workspace::{Workspace, WorkspaceCommand, WorkspaceEffect};
use regie::{build_icon, instance, terminal_profile, APP_NAME};
use std::collections::VecDeque;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
#[cfg(target_os = "macos")]
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

#[derive(Debug)]
enum AppEvent {
    Hotkey(GlobalHotKeyEvent),
    TrayClick,
    ShowRequest,
    Request(String),
    Wake,
    Menu(AppCommand),
    Settings(SettingsRequest),
    SettingsWatch(SettingsWatchEvent),
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1)
}

fn main() {
    let home = std::env::var_os("HOME").unwrap_or_else(|| fail("HOME is required"));
    let home = Path::new(&home).to_path_buf();
    let paths = AppPaths::new(&home);
    let instance = match instance::claim(&paths.runtime) {
        Ok(Some(instance)) => instance,
        Ok(None) => {
            let _ = instance::request_show(&paths.runtime);
            return;
        }
        Err(error) => fail(&format!("could not guard against a second copy: {error}")),
    };
    let snapshot = SettingsStore::read(&paths);
    let mut config = match &snapshot.status {
        ConfigStatus::Valid(config) => config.clone(),
        _ => config::Config::default(),
    };
    let mut settings = SettingsController::new(snapshot, config.clone());
    let mut hotkey = config.hotkey().unwrap_or_else(|e| fail(&e));
    let resolved = regie::theme::resolve(
        config.terminal_theme,
        terminal_profile::from_default_terminal(),
        terminal_profile::default_font(),
    );
    for notice in resolved.notices {
        eprintln!("{notice}");
    }
    let theme = resolved.theme;
    let mut event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    #[cfg(target_os = "macos")]
    event_loop.set_activation_policy(ActivationPolicy::Accessory);
    let proxy = event_loop.create_proxy();
    let modal = Arc::new(AtomicBool::new(false));
    let hotkey_proxy = proxy.clone();
    let hotkey_modal = modal.clone();
    GlobalHotKeyEvent::set_event_handler(Some(move |event| {
        if !hotkey_modal.load(Ordering::Acquire) {
            let _ = hotkey_proxy.send_event(AppEvent::Hotkey(event));
        }
    }));
    let show_proxy = proxy.clone();
    instance
        .listen(move || {
            let _ = show_proxy.send_event(AppEvent::ShowRequest);
        })
        .unwrap_or_else(|e| fail(&e.to_string()));
    let output = SessionOutput::new();
    let wake_proxy = proxy.clone();
    output.set_waker(move || {
        let _ = wake_proxy.send_event(AppEvent::Wake);
    });
    let mut workspace = Workspace::new(config.clone(), home, paths.clone(), output);
    let mut dropdown: Option<DropDown> = None;
    let mut tray: Option<TrayIcon> = None;
    let mut ui: Option<AppUi> = None;
    let mut manager: Option<GlobalHotKeyManager> = None;
    let mut visibility = Visibility::Shown;
    let mut show_on_launch = false;
    let mut settings_window: Option<SettingsView> = None;
    let mut settings_ready = false;
    let mut focus_check_pending = false;
    let mut settings_warning: Option<String> = None;
    let mut warning_icon = false;
    let watch_proxy = proxy.clone();
    let watcher = SettingsWatch::new(paths.clone(), move |event| {
        let _ = watch_proxy.send_event(AppEvent::SettingsWatch(event));
    });
    let mut watcher_warning = watcher.as_ref().err().cloned();

    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        let mut setting_effects = Vec::new();
        let effects = match event {
            Event::NewEvents(StartCause::Init) => {
                let menu_proxy = proxy.clone();
                let menu_modal = modal.clone();
                let app_ui = AppUi::new(
                    move |command| {
                        if !menu_modal.load(Ordering::Acquire) {
                            let _ = menu_proxy.send_event(AppEvent::Menu(command));
                        }
                    },
                    config.appearance,
                )
                .unwrap_or_else(|e| fail(&e));
                let builder = TrayIconBuilder::new()
                    .with_menu(Box::new(app_ui.tray.clone()))
                    .with_menu_on_left_click(false)
                    .with_tooltip(APP_NAME);
                #[cfg(target_os = "macos")]
                let builder = builder.with_icon_templated(build_icon());
                #[cfg(not(target_os = "macos"))]
                let builder = builder.with_icon(build_icon());
                tray = Some(builder.build().unwrap_or_else(|e| fail(&e.to_string())));
                ui = Some(app_ui);
                let tray_proxy = proxy.clone();
                let tray_modal = modal.clone();
                TrayIconEvent::set_event_handler(Some(move |event| {
                    if !tray_modal.load(Ordering::Acquire)
                        && matches!(
                            event,
                            TrayIconEvent::Click {
                                button: MouseButton::Left,
                                button_state: MouseButtonState::Up,
                                ..
                            }
                        )
                    {
                        let _ = tray_proxy.send_event(AppEvent::TrayClick);
                    }
                }));
                let hotkey_manager =
                    GlobalHotKeyManager::new().unwrap_or_else(|e| fail(&e.to_string()));
                if let Err(error) = hotkey_manager.register(hotkey) {
                    settings_warning = Some(format!("Could not register shortcut: {error}"));
                }
                manager = Some(hotkey_manager);
                let ipc_proxy = proxy.clone();
                dropdown = Some(
                    DropDown::new(target, &config, &theme, move |body| {
                        let _ = ipc_proxy.send_event(AppEvent::Request(body));
                    })
                    .unwrap_or_else(|e| fail(&e)),
                );
                show_on_launch = true;
                setting_effects.extend(settings.initial_effects());
                workspace.handle(WorkspaceCommand::Start)
            }
            Event::UserEvent(AppEvent::Hotkey(event))
                if event.id() == hotkey.id() && should_toggle(event.state()) =>
            {
                toggle(&mut workspace, &mut visibility, &mut dropdown)
            }
            Event::UserEvent(AppEvent::TrayClick) => {
                toggle(&mut workspace, &mut visibility, &mut dropdown)
            }
            Event::UserEvent(AppEvent::ShowRequest) | Event::Reopen { .. } => {
                workspace.handle(WorkspaceCommand::Summon)
            }
            Event::UserEvent(AppEvent::Menu(AppCommand::OpenSettings)) => {
                setting_effects.push(SettingsEffect::OpenSettings);
                vec![]
            }
            Event::UserEvent(AppEvent::Menu(command)) => {
                workspace.handle(WorkspaceCommand::Native(command))
            }
            Event::UserEvent(AppEvent::Request(body)) => match parse_workspace_request(&body) {
                Ok(request) => workspace.handle(WorkspaceCommand::Ui(request)),
                Err(error) => {
                    eprintln!("{error}");
                    vec![]
                }
            },
            Event::UserEvent(AppEvent::Wake) => workspace.handle(WorkspaceCommand::Pump),
            Event::UserEvent(AppEvent::SettingsWatch(event)) => {
                match event {
                    SettingsWatchEvent::Changed => setting_effects
                        .extend(settings.external_change(SettingsStore::read(&paths))),
                    SettingsWatchEvent::Failed(error) => {
                        watcher_warning = Some(error);
                        setting_effects.push(SettingsEffect::Render);
                    }
                }
                vec![]
            }
            Event::UserEvent(AppEvent::Settings(request)) => {
                match request {
                    SettingsRequest::Ready => {
                        settings_ready = true;
                        setting_effects.push(SettingsEffect::Render);
                    }
                    SettingsRequest::Edit { key, value } => {
                        if let Ok(key) = settings_view::decode_key(&key) {
                            if let Ok(value) = settings_view::decode_value(key, value) {
                                setting_effects.extend(settings.edit(key, value));
                            }
                        }
                    }
                    SettingsRequest::ResetOne { key } => {
                        if let Ok(key) = settings_view::decode_key(&key) {
                            if let Some(value) =
                                regie::settings::value_for(&config::Config::default(), key)
                            {
                                setting_effects.extend(settings.edit(key, value));
                            }
                        }
                    }
                    SettingsRequest::ResetAll => setting_effects.extend(settings.reset_all()),
                    SettingsRequest::Retry => setting_effects.extend(settings.retry()),
                    SettingsRequest::Close => {
                        if let Some(window) = &settings_window {
                            window.hide();
                        }
                    }
                    SettingsRequest::OpenFile => {
                        let _ = std::process::Command::new("open")
                            .arg(&settings.snapshot().path)
                            .spawn();
                    }
                }
                vec![]
            }
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
                ..
            } if settings_window
                .as_ref()
                .is_some_and(|window| window.window_id() == window_id) =>
            {
                if let Some(window) = &settings_window {
                    window.hide();
                }
                vec![]
            }
            Event::WindowEvent {
                window_id,
                event: WindowEvent::Resized(_),
                ..
            } => {
                if let Some(d) = dropdown.as_ref().filter(|d| d.window_id() == window_id) {
                    let _ = d.resize_terminal_view();
                }
                vec![]
            }
            Event::WindowEvent {
                window_id,
                event: WindowEvent::Focused(false),
                ..
            } => {
                if dropdown
                    .as_ref()
                    .is_some_and(|d| d.window_id() == window_id)
                    || settings_window
                        .as_ref()
                        .is_some_and(|w| w.window_id() == window_id)
                {
                    focus_check_pending = true;
                }
                vec![]
            }
            Event::MainEventsCleared => {
                if focus_check_pending {
                    focus_check_pending = false;
                    if workspace.hide_on_focus_loss()
                        && visibility == Visibility::Shown
                        && !settings_window
                            .as_ref()
                            .is_some_and(SettingsView::is_focused)
                        && dropdown.as_ref().is_some_and(|d| !d.is_focused())
                    {
                        visibility = Visibility::Hidden;
                        if let Some(d) = dropdown.as_mut() {
                            d.hide(Instant::now());
                        }
                    }
                }
                if let Some(d) = dropdown.as_mut() {
                    if show_on_launch {
                        show_on_launch = false;
                        d.show(Instant::now());
                    }
                    d.tick(Instant::now());
                }
                workspace.handle(WorkspaceCommand::Pump)
            }
            _ => vec![],
        };
        let mut pending: VecDeque<_> = setting_effects.into();
        while let Some(effect) = pending.pop_front() {
            match effect {
                SettingsEffect::OpenSettings => {
                    if settings_window.is_none() {
                        let settings_proxy = proxy.clone();
                        match SettingsView::new(target, move |request| {
                            let _ = settings_proxy.send_event(AppEvent::Settings(request));
                        }) {
                            Ok(window) => settings_window = Some(window),
                            Err(error) => {
                                settings_warning = Some(error);
                            }
                        }
                    }
                    if let Some(window) = &settings_window {
                        window.show();
                    }
                }
                SettingsEffect::ShowWarning(message) | SettingsEffect::ShowError(message) => {
                    eprintln!("{message}");
                    settings_warning = Some(message);
                }
                SettingsEffect::ClearWarning => settings_warning = None,
                SettingsEffect::Render => {
                    if settings_ready {
                        if let Some(window) = &settings_window {
                            if let Err(error) = window.render(&settings_view::view_state(
                                &settings,
                                settings_warning.clone().or_else(|| watcher_warning.clone()),
                            )) {
                                eprintln!("{error}");
                            }
                        }
                    }
                }
                SettingsEffect::Write {
                    generation,
                    changes,
                } => {
                    let expected = settings.snapshot().clone();
                    let replacement = changes.iter().find_map(|(key, value)| match (key, value) {
                        (SettingKey::Hotkey, regie::settings_store::SettingValue::String(text)) => {
                            regie::hotkey::parse_hotkey(text).ok()
                        }
                        _ => None,
                    });
                    let result = if let Some(new) = replacement.filter(|new| *new != hotkey) {
                        if let Some(manager) = manager.as_mut() {
                            let mut written = None;
                            let outcome =
                                regie::hotkey::replace_hotkey(manager, hotkey, new, || {
                                    let snapshot =
                                        SettingsStore::write(&paths, &expected, &changes)
                                            .map_err(|e| format!("{e:?}"))?;
                                    written = Some(snapshot);
                                    Ok(())
                                });
                            match outcome {
                                Ok(()) => {
                                    let snapshot = written.expect("successful write");
                                    if matches!(snapshot.status, ConfigStatus::Valid(_)) {
                                        hotkey = new;
                                        Ok(snapshot)
                                    } else {
                                        // The candidate was tested and saved, but invalid files must
                                        // keep the previously active configuration until repaired.
                                        match regie::hotkey::replace_hotkey(manager, new, hotkey, || Ok(())) {
                                            Ok(()) => Ok(snapshot),
                                            Err(error) => {
                                                hotkey = new;
                                                Err(format!("Settings were saved, but restoring the previous shortcut failed: {error}"))
                                            }
                                        }
                                    }
                                }
                                Err(error) => Err(error.to_string()),
                            }
                        } else {
                            Err("Shortcut manager unavailable".into())
                        }
                    } else {
                        SettingsStore::write(&paths, &expected, &changes)
                            .map_err(|e| format!("{e:?}"))
                    };
                    pending.extend(settings.finish(SettingsResult::Write { generation, result }));
                }
                SettingsEffect::Apply {
                    generation,
                    config: next,
                } => {
                    let result = (|| -> Result<(), String> {
                        let new_hotkey = next.hotkey()?;
                        if new_hotkey != hotkey {
                            let manager = manager.as_mut().ok_or("Shortcut manager unavailable")?;
                            regie::hotkey::replace_hotkey(manager, hotkey, new_hotkey, || Ok(()))
                                .map_err(|e| e.to_string())?;
                            hotkey = new_hotkey;
                        }
                        let theme = regie::theme::resolve(
                            next.terminal_theme,
                            terminal_profile::from_default_terminal(),
                            terminal_profile::default_font(),
                        )
                        .theme;
                        if let Some(dropdown) = dropdown.as_mut() {
                            dropdown.apply_config(&next, &theme)?;
                        }
                        app_ui::apply_appearance(next.appearance);
                        let _ = workspace.apply_config(&next);
                        config = next;
                        Ok(())
                    })();
                    pending.extend(settings.finish(SettingsResult::Apply { generation, result }));
                }
            }
        }
        let warning = settings_warning.is_some() || watcher_warning.is_some();
        if warning != warning_icon {
            if let Some(menu) = &ui {
                menu.set_config_warning(warning);
            }
            if let Some(icon) = &tray {
                let result = icon.set_icon(Some(if warning {
                    regie::build_warning_icon()
                } else {
                    build_icon()
                }));
                if let Err(error) = result {
                    eprintln!("could not update menu icon: {error}");
                }
            }
            warning_icon = warning;
        }
        let mut effects: VecDeque<_> = effects.into();
        while let Some(effect) = effects.pop_front() {
            match effect {
                WorkspaceEffect::Ui(deliveries) => {
                    let output = deliveries
                        .iter()
                        .any(|d| matches!(d, UiDelivery::Output { .. } | UiDelivery::End { .. }));
                    let result = dropdown
                        .as_mut()
                        .ok_or_else(|| "window not created".to_string())
                        .and_then(|d| d.apply_ui(&deliveries));
                    if output {
                        workspace.finish_delivery(result.is_ok());
                    }
                    if let Err(error) = result {
                        eprintln!("{error}");
                    }
                }
                WorkspaceEffect::ShowPanel => {
                    visibility = Visibility::Shown;
                    if let Some(d) = dropdown.as_mut() {
                        d.show(Instant::now());
                    }
                }
                WorkspaceEffect::HidePanel => {
                    visibility = Visibility::Hidden;
                    if let Some(d) = dropdown.as_mut() {
                        d.hide(Instant::now());
                    }
                }
                WorkspaceEffect::Confirm { token, names } => {
                    modal.store(true, Ordering::Release);
                    let confirmed = app_ui::confirm_termination(&names, config.appearance);
                    effects
                        .extend(workspace.handle(WorkspaceCommand::Confirm { token, confirmed }));
                    modal.store(false, Ordering::Release);
                    if visibility == Visibility::Shown {
                        if let Some(d) = dropdown.as_mut() {
                            d.show(Instant::now());
                        }
                    }
                }
                WorkspaceEffect::Copy(text) => {
                    if let Err(error) = clipboard::write(&text) {
                        eprintln!("{error}");
                    }
                }
                WorkspaceEffect::ReadPaste(id) => match clipboard::read() {
                    Ok(Some(text)) => {
                        effects.extend(workspace.handle(WorkspaceCommand::Paste { id, text }))
                    }
                    Ok(None) => {}
                    Err(error) => eprintln!("{error}"),
                },
                WorkspaceEffect::Exit => *control_flow = ControlFlow::Exit,
            }
        }
        if !matches!(*control_flow, ControlFlow::Exit)
            && dropdown.as_ref().is_some_and(DropDown::is_animating)
        {
            *control_flow = ControlFlow::Poll;
        }
        let _ = (&tray, &ui, &manager, &instance, &watcher);
    });
}

fn toggle(
    workspace: &mut Workspace,
    visibility: &mut Visibility,
    dropdown: &mut Option<DropDown>,
) -> Vec<WorkspaceEffect> {
    if workspace.confirmation_pending() {
        return vec![];
    }
    if *visibility == Visibility::Shown {
        *visibility = Visibility::Hidden;
        if let Some(d) = dropdown.as_mut() {
            d.hide(Instant::now());
        }
        vec![]
    } else {
        workspace.handle(WorkspaceCommand::Summon)
    }
}
