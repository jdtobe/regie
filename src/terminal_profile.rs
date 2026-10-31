//! Reads macOS Terminal colors and font independently, without changing preferences.
//!
//! This is OS glue and is verified by hand rather than unit tested: the decoding was checked
//! against a real profile (background `#212121`, text `#d0d0d0`, Menlo 11pt for Afterglow).

use crate::theme::TerminalFont;
use crate::theme::TerminalTheme;

pub fn default_font() -> TerminalFont {
    #[cfg(target_os = "macos")]
    {
        macos::default_font().unwrap_or_default()
    }
    #[cfg(not(target_os = "macos"))]
    {
        TerminalFont::default()
    }
}

/// The theme of the macOS Terminal default profile, if it can be read. On other platforms, or
/// when the profile cannot be decoded, callers keep Rake's default look.
#[cfg(target_os = "macos")]
pub fn from_default_terminal() -> Option<TerminalTheme> {
    macos::from_default_terminal()
}

#[cfg(not(target_os = "macos"))]
pub fn from_default_terminal() -> Option<TerminalTheme> {
    None
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::rc::Retained;
    use objc2::{AnyThread, ClassType, DowncastTarget};
    use objc2_app_kit::{NSColor, NSColorSpace, NSFont};
    use objc2_foundation::{NSData, NSDictionary, NSKeyedUnarchiver, NSString, NSUserDefaults};

    use crate::theme::{Rgb, TerminalFont, TerminalTheme};

    /// Terminal.app's profile keys for the 16 ANSI colours, normal then bright.
    const ANSI_KEYS: [&str; 16] = [
        "ANSIBlackColor",
        "ANSIRedColor",
        "ANSIGreenColor",
        "ANSIYellowColor",
        "ANSIBlueColor",
        "ANSIMagentaColor",
        "ANSICyanColor",
        "ANSIWhiteColor",
        "ANSIBrightBlackColor",
        "ANSIBrightRedColor",
        "ANSIBrightGreenColor",
        "ANSIBrightYellowColor",
        "ANSIBrightBlueColor",
        "ANSIBrightMagentaColor",
        "ANSIBrightCyanColor",
        "ANSIBrightWhiteColor",
    ];

    fn key(name: &str) -> Retained<NSString> {
        NSString::from_str(name)
    }

    fn defaults() -> Option<Retained<NSUserDefaults>> {
        NSUserDefaults::initWithSuiteName(NSUserDefaults::alloc(), Some(&key("com.apple.Terminal")))
    }

    fn default_profile(defaults: &NSUserDefaults) -> Option<Retained<NSDictionary>> {
        let name = defaults.stringForKey(&key("Default Window Settings"))?;
        let settings = defaults.dictionaryForKey(&key("Window Settings"))?;
        let profile = settings.objectForKey(&name)?;
        profile.downcast::<NSDictionary>().ok()
    }

    fn archived<T>(profile: &NSDictionary, name: &str) -> Option<Retained<T>>
    where
        T: ClassType + DowncastTarget,
    {
        let value = profile.objectForKey(&key(name))?;
        let data = value.downcast::<NSData>().ok()?;
        let object =
            unsafe { NSKeyedUnarchiver::unarchivedObjectOfClass_fromData_error(T::class(), &data) }
                .ok()?;
        object.downcast::<T>().ok()
    }

    fn color(profile: &NSDictionary, name: &str) -> Option<Rgb> {
        let color = archived::<NSColor>(profile, name)?;
        let srgb = color.colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())?;
        Some(Rgb::from_unit(
            srgb.redComponent(),
            srgb.greenComponent(),
            srgb.blueComponent(),
        ))
    }

    fn palette(profile: &NSDictionary) -> Option<[Rgb; 16]> {
        // Half a palette would look worse than xterm.js's own set, so take all or nothing.
        let mut ansi = [Rgb::new(0, 0, 0); 16];
        for (slot, name) in ansi.iter_mut().zip(ANSI_KEYS) {
            *slot = color(profile, name)?;
        }
        Some(ansi)
    }

    fn font(profile: &NSDictionary) -> TerminalFont {
        let (family, size) = archived::<NSFont>(profile, "Font")
            .and_then(|font| {
                let family = font.familyName()?.to_string();
                Some((
                    Some(family),
                    Some(TerminalTheme::css_font_size(font.pointSize() as f32)),
                ))
            })
            .unwrap_or_default();
        TerminalFont { family, size }
    }

    pub(super) fn default_font() -> Option<TerminalFont> {
        let defaults = defaults()?;
        let profile = default_profile(&defaults)?;
        Some(font(&profile))
    }

    pub(super) fn from_default_terminal() -> Option<TerminalTheme> {
        let defaults = defaults()?;
        let profile = default_profile(&defaults)?;
        let fallback = TerminalTheme::profile_fallback();
        let font = font(&profile);
        Some(TerminalTheme {
            background: color(&profile, "BackgroundColor").unwrap_or(fallback.background),
            foreground: color(&profile, "TextColor"),
            cursor: color(&profile, "CursorColor"),
            ansi: palette(&profile),
            font_family: font.family,
            font_size: font.size,
            ..fallback
        })
    }
}
