//! Pure helpers for the menubar app, kept separate from the event loop so they can be unit tested.

pub mod animation;
pub mod app_paths;
pub mod app_ui;
pub mod clipboard;
pub mod close_policy;
pub mod config;
pub mod dropdown;
pub mod geometry;
pub mod hotkey;
pub mod instance;
pub mod process;
pub mod pty;
pub mod session_output;
pub mod sessions;
pub mod settings;
pub mod settings_store;
pub mod settings_view;
pub mod settings_watch;
pub mod tab_layout;
pub mod tabs;
pub mod terminal_profile;
pub mod terminal_protocol;
pub mod terminal_view;
pub mod theme;
pub mod workspace;

use tray_icon::menu::AboutMetadata;
use tray_icon::menu::Icon as AboutIcon;
use tray_icon::Icon;

/// Pixel size of the menubar icon. The drawing grid is 22 units; drawing at twice that
/// gives Retina real pixels, and the tray scales the image down to 22 points.
pub const ICON_SIZE: u32 = 44;
#[cfg(test)]
const ICON_SCALE: f32 = 2.0;
pub const APP_NAME: &str = "Regie";

/// RGBA pixels for the menubar Descending Cue, black with alpha
/// so macOS can use it as a template image and tint it for light and dark menubars.
pub fn build_icon_rgba() -> Vec<u8> {
    let (rgba, width, height) = decode_icon(include_bytes!("../assets/icons/menubar-44.png"));
    assert_eq!((width, height), (ICON_SIZE, ICON_SIZE));
    rgba
}

pub fn build_icon() -> Icon {
    Icon::from_rgba(build_icon_rgba(), ICON_SIZE, ICON_SIZE).expect("icon buffer matches size")
}

pub fn build_warning_icon_rgba() -> Vec<u8> {
    let mut pixels = build_icon_rgba();
    for y in 2..12 {
        for x in 32..42 {
            if (x as i32 - 37).pow(2) + (y as i32 - 7).pow(2) <= 25 {
                let offset = ((y * ICON_SIZE + x) * 4) as usize;
                pixels[offset..offset + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
    }
    pixels
}

pub fn build_warning_icon() -> Icon {
    Icon::from_rgba(build_warning_icon_rgba(), ICON_SIZE, ICON_SIZE)
        .expect("icon buffer matches size")
}

/// The colour app icon, compiled in from the same master that produces the menubar mark.
const APP_ICON_PNG: &[u8] = include_bytes!("../assets/icons/app-1024.png");

/// Decode the app icon to RGBA pixels: `(pixels, width, height)`.
fn app_icon_rgba() -> (Vec<u8>, u32, u32) {
    decode_icon(APP_ICON_PNG)
}

fn decode_icon(bytes: &[u8]) -> (Vec<u8>, u32, u32) {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .expect("the app icon is a valid PNG");
    let mut buffer = vec![
        0;
        reader
            .output_buffer_size()
            .expect("the icon fits in memory")
    ];
    let info = reader
        .next_frame(&mut buffer)
        .expect("the app icon has one frame");
    buffer.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buffer,
        png::ColorType::Rgb => buffer
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect(),
        other => panic!("the app icon must be RGB or RGBA, not {other:?}"),
    };
    (rgba, info.width, info.height)
}

/// What the native About panel shows: name, version, copyright, credits and the colour icon.
pub fn about_metadata() -> AboutMetadata {
    let (icon_rgba, width, height) = app_icon_rgba();
    AboutMetadata {
        name: Some(APP_NAME.to_string()),
        version: Some(format!("v{}", env!("CARGO_PKG_VERSION"))),
        copyright: Some("© 2026 Jonathan Tobe".to_string()),
        credits: Some(
            "A drop-down terminal for macOS.\n\nBuilt with Rust, tao, wry and xterm.js.\nMIT licensed."
                .to_string(),
        ),
        icon: Some(
            AboutIcon::from_rgba(icon_rgba, width, height).expect("the app icon matches its size"),
        ),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Alpha at a point on the 22-unit drawing grid, sampled at its 2x pixel.
    fn alpha_at_grid_point(data: &[u8], x: f32, y: f32) -> u8 {
        let px = (x * ICON_SCALE) as u32;
        let py = (y * ICON_SCALE) as u32;
        data[((py * ICON_SIZE + px) * 4 + 3) as usize]
    }

    #[test]
    fn icon_rgba_has_expected_length() {
        assert_eq!(
            build_icon_rgba().len(),
            (ICON_SIZE * ICON_SIZE * 4) as usize
        );
    }

    #[test]
    fn config_warning_icon_adds_a_visible_template_badge() {
        let normal = build_icon_rgba();
        let warning = build_warning_icon_rgba();
        assert_eq!(warning.len(), normal.len());
        assert_ne!(warning, normal);
        assert!(warning
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[0..3] == [0, 0, 0]));
    }

    #[test]
    fn icon_is_drawn_at_two_times_for_retina() {
        // 44 pixels over the 22-unit grid; the tray scales it down to 22 points.
        assert_eq!(build_icon_rgba().len(), (44 * 44 * 4) as usize);
    }

    #[test]
    fn icon_draws_continuous_brackets_and_centered_descending_cue() {
        let data = build_icon_rgba();
        for (x, y) in [(4.0, 6.0), (18.0, 6.0), (11.0, 10.25), (11.0, 14.25)] {
            assert!(alpha_at_grid_point(&data, x, y) >= 240, "{x},{y}");
        }
    }

    #[test]
    fn icon_leaves_cue_gap_and_top_center_empty() {
        let data = build_icon_rgba();
        for (x, y) in [(11.0, 12.0), (11.0, 5.0), (0.0, 0.0), (21.0, 21.0)] {
            assert_eq!(alpha_at_grid_point(&data, x, y), 0, "{x},{y}");
        }
    }

    #[test]
    fn app_icon_has_forest_cue_and_transparent_corners() {
        let (data, width, _) = app_icon_rgba();
        let pixel = |x: u32, y: u32| {
            &data[((y * width + x) * 4) as usize..((y * width + x) * 4 + 4) as usize]
        };
        assert_eq!(pixel(64 * 8, 60 * 8), &[43, 76, 59, 255]);
        assert_eq!(pixel(0, 0)[3], 0);
        assert_eq!(pixel(1023, 1023)[3], 0);
    }

    #[test]
    fn icon_uses_only_black_for_template_tinting() {
        for pixel in build_icon_rgba().chunks(4) {
            if pixel[3] != 0 {
                assert_eq!(&pixel[..3], &[0, 0, 0]);
            }
        }
    }

    #[test]
    fn icon_rgba_is_not_fully_transparent() {
        assert!(build_icon_rgba().chunks(4).any(|px| px[3] != 0));
    }

    #[test]
    fn icon_is_accepted_by_tray_icon() {
        // Panics if the buffer does not match the declared dimensions.
        let _ = build_icon();
    }

    #[test]
    fn app_icon_decodes_to_rgba_pixels() {
        let (rgba, width, height) = app_icon_rgba();
        assert_eq!((width, height), (1024, 1024));
        assert_eq!(rgba.len(), (width * height * 4) as usize);
        assert!(rgba.chunks(4).any(|pixel| pixel[3] != 0));
    }

    #[test]
    fn about_metadata_uses_regie_brand() {
        let meta = about_metadata();
        assert_eq!(meta.name.as_deref(), Some("Regie"));
        assert_eq!(
            meta.version,
            Some(format!("v{}", env!("CARGO_PKG_VERSION")))
        );
        assert_eq!(meta.copyright.as_deref(), Some("© 2026 Jonathan Tobe"));
        assert!(meta
            .credits
            .as_deref()
            .is_some_and(|credits| credits.contains("xterm.js")));
        assert!(meta.icon.is_some());
    }
}
