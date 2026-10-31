//! Main-thread OS wiring; session and tab policy live in Workspace.
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use regie::app_paths::AppPaths;
use regie::app_ui::{self, AppCommand, AppUi};
use regie::clipboard;
use regie::config;
use regie::dropdown::DropDown;
use regie::hotkey::{should_toggle, Visibility};
use regie::session_output::SessionOutput;
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
    let loaded = config::load_compatible(&paths).unwrap_or_else(|e| fail(&e));
    for notice in loaded.notices {
        eprintln!("{notice}");
    }
    let config = loaded.config;
    let hotkey = config.hotkey().unwrap_or_else(|e| fail(&e));
    let theme = terminal_profile::from_default_terminal().unwrap_or_default();
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
    let mut workspace = Workspace::new(config.clone(), home, paths, output);
    let mut dropdown: Option<DropDown> = None;
    let mut tray: Option<TrayIcon> = None;
    let mut ui: Option<AppUi> = None;
    let mut manager: Option<GlobalHotKeyManager> = None;
    let mut visibility = Visibility::Shown;
    let mut show_on_launch = false;

    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
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
                hotkey_manager
                    .register(hotkey)
                    .unwrap_or_else(|e| fail(&e.to_string()));
                manager = Some(hotkey_manager);
                let ipc_proxy = proxy.clone();
                dropdown = Some(
                    DropDown::new(target, &config, &theme, move |body| {
                        let _ = ipc_proxy.send_event(AppEvent::Request(body));
                    })
                    .unwrap_or_else(|e| fail(&e)),
                );
                show_on_launch = true;
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
                if workspace.hide_on_focus_loss() && visibility == Visibility::Shown {
                    if let Some(d) = dropdown
                        .as_mut()
                        .filter(|d| d.window_id() == window_id && !d.is_focused())
                    {
                        visibility = Visibility::Hidden;
                        d.hide(Instant::now());
                    }
                }
                vec![]
            }
            Event::MainEventsCleared => {
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
        let _ = (&tray, &ui, &manager, &instance);
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
