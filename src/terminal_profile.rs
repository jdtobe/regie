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

    fn palette(colors: [Option<Rgb>; 16]) -> Option<[Rgb; 16]> {
        // Half a palette would look worse than xterm.js's own set, so take all or nothing.
        let mut ansi = [Rgb::new(0, 0, 0); 16];
        for (slot, color) in ansi.iter_mut().zip(colors) {
            *slot = color?;
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
        from_profile(&profile)
    }

    fn from_profile(profile: &NSDictionary) -> Option<TerminalTheme> {
        let background = color(profile, "BackgroundColor");
        let foreground = color(profile, "TextColor");
        let cursor = color(profile, "CursorColor");
        let ansi = ANSI_KEYS.map(|key| color(profile, key));
        if background.is_none()
            && foreground.is_none()
            && cursor.is_none()
            && ansi.iter().all(Option::is_none)
        {
            return None;
        }
        let fallback = TerminalTheme::profile_fallback();
        let font = font(profile);
        Some(TerminalTheme {
            background: background.unwrap_or(fallback.background),
            foreground,
            cursor,
            ansi: palette(ansi),
            font_family: font.family,
            font_size: font.size,
            ..fallback
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use objc2::runtime::AnyObject;
        use objc2_foundation::NSKeyedArchiver;

        fn archive(object: &AnyObject) -> Retained<NSData> {
            unsafe {
                NSKeyedArchiver::archivedDataWithRootObject_requiringSecureCoding_error(
                    object, true,
                )
            }
            .unwrap()
        }

        #[test]
        fn undecodable_color_archives_trigger_regie_fallback_but_keep_font() {
            objc2::rc::autoreleasepool(|_| {
                let native_font = NSFont::fontWithName_size(&key("Menlo"), 11.0).unwrap();
                let font_data = archive(&native_font);
                // A valid archive of the wrong class is undecodable as NSColor.
                let wrong_class = archive(&key("not a color"));
                let keys: Vec<_> = ["BackgroundColor", "TextColor", "CursorColor", "Font"]
                    .map(key)
                    .into();
                let profile = NSDictionary::from_slices(
                    &keys.iter().map(|k| &**k).collect::<Vec<_>>(),
                    &[
                        &*wrong_class as &AnyObject,
                        &*wrong_class,
                        &*wrong_class,
                        &*font_data,
                    ],
                );
                // Erase key generics to match the untyped Terminal preferences reader.
                let profile: Retained<NSDictionary> = unsafe { Retained::cast_unchecked(profile) };
                let decoded = from_profile(&profile);
                assert!(
                    decoded.is_none(),
                    "no decodable colors must not count as a usable color profile"
                );
                let resolved = crate::theme::resolve(
                    crate::config::TerminalThemeSource::Macos,
                    decoded,
                    font(&profile),
                );
                assert_eq!(resolved.theme.background.hex(), "#121615");
                assert_eq!(resolved.theme.font_family.as_deref(), Some("Menlo"));
                assert_eq!(resolved.theme.font_size, Some(14.7));
                assert!(resolved.notices.join(" ").contains("Regie Dark"));
            });
        }

        #[test]
        fn one_readable_profile_color_keeps_neutral_partial_defaults() {
            objc2::rc::autoreleasepool(|_| {
                let color = NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 0.0, 0.0, 1.0);
                let data = archive(&color);
                let profile =
                    NSDictionary::from_slices(&[&*key("TextColor")], &[&*data as &AnyObject]);
                let profile: Retained<NSDictionary> = unsafe { Retained::cast_unchecked(profile) };
                let theme = from_profile(&profile).unwrap();
                assert_eq!(theme.background.hex(), "#1e1e1e");
                assert_eq!(theme.foreground.unwrap().hex(), "#ff0000");
                assert_eq!(theme.selection_background, None);
                assert_eq!(theme.cursor_accent, None);
            });
        }
    }
}
