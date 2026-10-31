//! The drop-down window. This is OS glue around `tao` and the terminal webview, so it is verified
//! by hand rather than unit tested; placement maths lives in [`crate::geometry`] and slide maths
//! in [`crate::animation`].

use std::rc::Rc;
use std::time::{Duration, Instant};

use tao::dpi::{PhysicalPosition, PhysicalSize};
use tao::event_loop::EventLoopWindowTarget;
use tao::window::{Window, WindowBuilder, WindowId};

use crate::animation::Slide;
use crate::config::Config;
use crate::geometry::{window_rect, Display, Rect};
use crate::terminal_protocol::UiDelivery;
use crate::terminal_view::TerminalView;
use crate::theme::TerminalTheme;
use crate::APP_NAME;

fn resized_panel(display: Display, width_percent: u8, height_percent: u8) -> (Rect, i32) {
    let home = window_rect(
        display.position,
        display.size,
        display.top_inset,
        width_percent,
        height_percent,
    );
    let hidden_y = display.position.1 - home.height as i32;
    (home, hidden_y)
}

/// macOS keeps normal and floating windows below the menubar and silently clamps any frame that
/// would go above it, which flattens the whole slide into one position. A window at the menubar's
/// own level is not clamped, and the menubar is drawn in front of it, so the panel slides out
/// from under the menubar rather than over it.
#[cfg(target_os = "macos")]
mod macos {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSEvent, NSScreen, NSWindow, NSWindowCollectionBehavior};
    use tao::platform::macos::WindowExtMacOS;
    use tao::window::Window;

    use crate::geometry::{display_from_frame, screen_at, Display, ScreenFrame};

    /// `NSMainMenuWindowLevel`. One level lower (23) is clamped; one higher (25, status) draws
    /// over the menubar.
    const MENUBAR_LEVEL: isize = 24;

    fn ns_window(window: &Window) -> &NSWindow {
        // SAFETY: tao returns the window's live `NSWindow`, which outlives the borrow of `window`.
        unsafe { &*window.ns_window().cast::<NSWindow>() }
    }

    pub fn configure_panel(window: &Window) {
        let window = ns_window(window);
        window.setLevel(MENUBAR_LEVEL);
        window.setHidesOnDeactivate(false);
        // The panel belongs to a display, not a Space: show it on every Space and beside
        // full-screen apps, so the hotkey works wherever that display currently is.
        let behavior = window.collectionBehavior()
            | NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary;
        window.setCollectionBehavior(behavior);
    }

    /// The display under the mouse, in the physical coordinates the window uses.
    pub fn display_under_cursor() -> Option<Display> {
        let marker = MainThreadMarker::new()?;
        let screens = NSScreen::screens(marker);
        if screens.is_empty() {
            return None;
        }
        let frames: Vec<ScreenFrame> = (0..screens.len())
            .map(|index| screen_frame(&screens.objectAtIndex(index)))
            .collect();
        let cursor = NSEvent::mouseLocation();
        let index = screen_at((cursor.x as f32, cursor.y as f32), &frames)?;
        let screen = screens.objectAtIndex(index);
        let frame = screen.frame();
        let usable = screen.visibleFrame();
        let top_inset =
            (frame.origin.y + frame.size.height) - (usable.origin.y + usable.size.height);
        let primary_height = screen_frame(&screens.objectAtIndex(0)).height;
        Some(display_from_frame(
            frames[index],
            top_inset as f32,
            screen.backingScaleFactor(),
            primary_height,
        ))
    }

    fn screen_frame(screen: &NSScreen) -> ScreenFrame {
        let frame = screen.frame();
        ScreenFrame {
            x: frame.origin.x as f32,
            y: frame.origin.y as f32,
            width: frame.size.width as f32,
            height: frame.size.height as f32,
        }
    }

    pub fn focus(window: &Window) {
        let marker = MainThreadMarker::new().expect("GUI work runs on the main thread");
        NSApplication::sharedApplication(marker).activate();
        ns_window(window).makeKeyAndOrderFront(None);
        window.set_focus();
    }

    /// Height in physical pixels of the strip at the top of the window's screen that is not
    /// usable (the menubar), or 0 if the screen is unknown.
    pub fn top_inset_px(window: &Window) -> u32 {
        let ns_window = ns_window(window);
        let Some(screen) = ns_window.screen() else {
            return 0;
        };
        let (frame, usable) = (screen.frame(), screen.visibleFrame());
        let points = (frame.origin.y + frame.size.height) - (usable.origin.y + usable.size.height);
        (points.max(0.0) * ns_window.backingScaleFactor()).round() as u32
    }
}

#[cfg(not(target_os = "macos"))]
mod macos {
    use tao::window::Window;

    use crate::geometry::Display;

    pub fn configure_panel(_window: &Window) {}

    pub fn display_under_cursor() -> Option<Display> {
        None
    }

    pub fn focus(window: &Window) {
        window.set_focus();
    }

    pub fn top_inset_px(_window: &Window) -> u32 {
        0
    }
}

pub struct DropDown {
    window: Rc<Window>,
    terminal: TerminalView,
    width_percent: u8,
    height_percent: u8,
    animation: Duration,
    /// Where the window rests while shown. Re-computed on every show from the display under
    /// the cursor.
    home: Option<Rect>,
    /// Top edge of the window when parked fully above its monitor, in physical pixels.
    hidden_y: i32,
    /// Current top edge of the window, in physical pixels.
    y: i32,
    visible: bool,
    /// The running slide is a slide out, so the window is hidden when it finishes.
    hiding: bool,
    slide: Option<Slide>,
}

impl DropDown {
    /// Create the window hidden.
    pub fn new<T: 'static, F>(
        target: &EventLoopWindowTarget<T>,
        config: &Config,
        theme: &TerminalTheme,
        ipc: F,
    ) -> Result<Self, String>
    where
        F: Fn(String) + 'static,
    {
        let window = WindowBuilder::new()
            .with_title(APP_NAME)
            .with_decorations(false)
            .with_resizable(false)
            .with_always_on_top(true)
            .with_visible(false)
            .build(target)
            .map_err(|e| format!("could not create window: {e}"))?;
        macos::configure_panel(&window);
        let window = Rc::new(window);
        let terminal = TerminalView::new(&window, theme, config, ipc)?;
        Ok(DropDown {
            window,
            terminal,
            width_percent: config.width_percent,
            height_percent: config.height_percent,
            animation: Duration::from_millis(config.animation_ms.into()),
            home: None,
            hidden_y: 0,
            y: 0,
            visible: false,
            hiding: false,
            slide: None,
        })
    }

    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }

    /// True while a slide is running, so the event loop should keep waking up.
    pub fn is_animating(&self) -> bool {
        self.slide.is_some()
    }

    /// The display under the cursor, or the window's current monitor if the platform cannot
    /// answer.
    fn display_under_cursor(&self) -> Option<Display> {
        macos::display_under_cursor().or_else(|| {
            let monitor = self
                .window
                .current_monitor()
                .or_else(|| self.window.primary_monitor())?;
            let position = monitor.position();
            let size = monitor.size();
            Some(Display {
                position: (position.x, position.y),
                size: (size.width, size.height),
                top_inset: macos::top_inset_px(&self.window),
            })
        })
    }

    /// Slide the window in on the display under the cursor, then focus it. If it is partway
    /// out, it reverses on the display it is already on.
    pub fn show(&mut self, now: Instant) {
        let home = match (self.visible, self.home) {
            (true, Some(home)) => home,
            _ => {
                let Some(display) = self.display_under_cursor() else {
                    self.window.set_visible(true);
                    self.window.set_focus();
                    return;
                };
                let (home, hidden_y) =
                    resized_panel(display, self.width_percent, self.height_percent);
                self.hidden_y = hidden_y;
                home
            }
        };
        self.home = Some(home);
        self.window
            .set_inner_size(PhysicalSize::new(home.width, home.height));

        if !self.visible {
            self.y = self.hidden_y;
            self.window
                .set_outer_position(PhysicalPosition::new(home.x, self.y));
            self.visible = true;
            self.window.set_visible(true);
        }
        self.hiding = false;
        self.slide = Some(Slide::toward(
            now,
            self.y,
            home.y,
            self.travel(home),
            self.animation,
        ));
        macos::focus(&self.window);
        let _ = self.terminal.focus();
        self.window.request_redraw();
        self.tick(now);
    }

    /// Slide the window back up and hide it. If it is partway in, it reverses from where it is.
    pub fn hide(&mut self, now: Instant) {
        let Some(home) = self.home.filter(|_| self.visible) else {
            return;
        };
        self.hiding = true;
        self.slide = Some(Slide::toward(
            now,
            self.y,
            self.hidden_y,
            self.travel(home),
            self.animation,
        ));
        self.tick(now);
    }

    /// Distance between the hidden and resting positions.
    fn travel(&self, home: Rect) -> u32 {
        home.y.abs_diff(self.hidden_y)
    }

    /// Advance the running slide, if any. Call on every wake-up while [`is_animating`].
    ///
    /// [`is_animating`]: DropDown::is_animating
    pub fn tick(&mut self, now: Instant) {
        let (Some(slide), Some(home)) = (self.slide, self.home) else {
            return;
        };
        self.y = slide.y_at(now);
        self.window
            .set_outer_position(PhysicalPosition::new(home.x, self.y));
        if !slide.is_done(now) {
            return;
        }
        self.slide = None;
        if self.hiding {
            self.hiding = false;
            self.visible = false;
            self.window.set_visible(false);
            // Park at the resting spot; nothing is visible while hidden.
            self.y = home.y;
            self.window
                .set_outer_position(PhysicalPosition::new(home.x, home.y));
        }
    }

    pub fn resize_terminal_view(&self) -> Result<(), String> {
        self.terminal
            .set_bounds(self.window.inner_size(), self.window.scale_factor())
    }

    pub fn apply_config(&mut self, config: &Config, theme: &TerminalTheme) -> Result<(), String> {
        let size_changed = self.width_percent != config.width_percent
            || self.height_percent != config.height_percent;
        self.width_percent = config.width_percent;
        self.height_percent = config.height_percent;
        self.animation = Duration::from_millis(config.animation_ms.into());
        if size_changed {
            if let Some(display) = self.display_under_cursor() {
                let home = window_rect(
                    display.position,
                    display.size,
                    display.top_inset,
                    self.width_percent,
                    self.height_percent,
                );
                self.hidden_y = display.position.1 - home.height as i32;
                self.home = Some(home);
                self.window
                    .set_inner_size(PhysicalSize::new(home.width, home.height));
                self.window
                    .set_outer_position(PhysicalPosition::new(home.x, self.y));
                if self.visible {
                    if self.slide.is_some() {
                        let target = if self.hiding { self.hidden_y } else { home.y };
                        self.slide = Some(Slide::toward(
                            Instant::now(),
                            self.y,
                            target,
                            home.y.abs_diff(self.hidden_y),
                            self.animation,
                        ));
                    } else {
                        self.y = home.y;
                        self.window
                            .set_outer_position(PhysicalPosition::new(home.x, home.y));
                    }
                }
            }
        }
        self.terminal.apply(&[
            UiDelivery::TerminalTheme {
                theme: theme.config_json(),
            },
            UiDelivery::TabPosition {
                position: config.tab_position,
            },
            UiDelivery::Appearance {
                appearance: config.appearance,
            },
        ])?;
        if self.visible {
            self.resize_terminal_view()?;
        }
        Ok(())
    }

    pub fn apply_ui(&mut self, deliveries: &[UiDelivery]) -> Result<(), String> {
        self.terminal.apply(deliveries)
    }
    pub fn is_focused(&self) -> bool {
        self.window.is_focused()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_resize_recomputes_resting_and_hidden_positions_from_new_percentages() {
        let display = Display {
            position: (0, 0),
            size: (1200, 800),
            top_inset: 24,
        };
        let (home, hidden_y) = resized_panel(display, 75, 60);
        assert_eq!(home.width, 900);
        assert_eq!(home.height, 465);
        assert_eq!(home.y, 24);
        assert_eq!(hidden_y, -465);
    }

    #[test]
    fn resize_animation_can_continue_from_current_position_to_new_endpoint() {
        let now = Instant::now();
        let (home, hidden_y) = resized_panel(
            Display {
                position: (0, 0),
                size: (1000, 800),
                top_inset: 20,
            },
            100,
            70,
        );
        let current_y = 3;
        let slide = Slide::toward(
            now,
            current_y,
            home.y,
            home.y.abs_diff(hidden_y),
            Duration::from_millis(200),
        );
        assert_eq!(slide.y_at(now), current_y);
        assert_eq!(slide.y_at(now + Duration::from_millis(500)), home.y);
    }
}
