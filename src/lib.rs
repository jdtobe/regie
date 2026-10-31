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
const ICON_SCALE: f32 = 2.0;
pub const APP_NAME: &str = "Regie";

/// RGBA pixels for the menubar icon: a filled bar above an outlined pane, black with alpha
/// so macOS can use it as a template image and tint it for light and dark menubars.
pub fn build_icon_rgba() -> Vec<u8> {
    let mut rgba = Vec::with_capacity((ICON_SIZE * ICON_SIZE * 4) as usize);
    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            rgba.extend_from_slice(&[0, 0, 0, coverage(x, y)]);
        }
    }
    rgba
}

/// Antialiased coverage of the mark at one pixel, sampled 4x4 per pixel.
fn coverage(x: u32, y: u32) -> u8 {
    const SAMPLES: u32 = 4;
    let mut hits = 0;
    for sample_y in 0..SAMPLES {
        for sample_x in 0..SAMPLES {
            let px = (x as f32 + (sample_x as f32 + 0.5) / SAMPLES as f32) / ICON_SCALE;
            let py = (y as f32 + (sample_y as f32 + 0.5) / SAMPLES as f32) / ICON_SCALE;
            if inside_bar(px, py) || inside_pane(px, py) {
                hits += 1;
            }
        }
    }
    ((255 * hits + SAMPLES * SAMPLES / 2) / (SAMPLES * SAMPLES)) as u8
}

/// Signed distance to a rounded rectangle, negative inside.
fn rounded_rect_distance(
    px: f32,
    py: f32,
    centre_x: f32,
    centre_y: f32,
    half_width: f32,
    half_height: f32,
    radius: f32,
) -> f32 {
    let qx = (px - centre_x).abs() - (half_width - radius);
    let qy = (py - centre_y).abs() - (half_height - radius);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - radius
}

/// The menu bar edge: x 3.5..18.5, y 4.5..6.9 on the 22-unit grid.
fn inside_bar(px: f32, py: f32) -> bool {
    rounded_rect_distance(px, py, 11.0, 5.7, 7.5, 1.2, 1.2) <= 0.0
}

/// The pane outline: x 5.5..16.5, y 9..18, stroke 1.6 centred on the edge.
fn inside_pane(px: f32, py: f32) -> bool {
    rounded_rect_distance(px, py, 11.0, 13.5, 5.5, 4.5, 1.8).abs() <= 0.8
}

pub fn build_icon() -> Icon {
    Icon::from_rgba(build_icon_rgba(), ICON_SIZE, ICON_SIZE).expect("icon buffer matches size")
}

/// The colour app icon, compiled in from the same master that produces the menubar mark.
const APP_ICON_PNG: &[u8] = include_bytes!("../assets/icons/app-1024.png");

/// Decode the app icon to RGBA pixels: `(pixels, width, height)`.
fn app_icon_rgba() -> (Vec<u8>, u32, u32) {
    let mut reader = png::Decoder::new(std::io::Cursor::new(APP_ICON_PNG))
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
    fn icon_is_drawn_at_two_times_for_retina() {
        // 44 pixels over the 22-unit grid; the tray scales it down to 22 points.
        assert_eq!(build_icon_rgba().len(), (44 * 44 * 4) as usize);
    }

    #[test]
    fn icon_draws_the_bar_at_the_top() {
        let data = build_icon_rgba();
        assert_eq!(alpha_at_grid_point(&data, 11.0, 5.7), 255);
    }

    #[test]
    fn icon_leaves_the_gap_between_bar_and_pane_empty() {
        let data = build_icon_rgba();
        assert_eq!(alpha_at_grid_point(&data, 11.0, 7.5), 0);
    }

    #[test]
    fn icon_outlines_the_pane() {
        let data = build_icon_rgba();
        assert_eq!(alpha_at_grid_point(&data, 11.0, 9.0), 255);
        assert_eq!(alpha_at_grid_point(&data, 11.0, 13.5), 0);
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
