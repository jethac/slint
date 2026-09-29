// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The width the text layout reports for a `font-variation-settings` value must be the
//! width the renderer actually inks: a `wdth` or `opsz` change can't measure one width
//! and paint another. For several axis settings this renders white text on black with
//! the software renderer and compares the rightmost inked column with the `width` the
//! `Text` element reported.

mod common;

use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, TargetPixel,
};
use std::rc::Rc;

const WIDTH: u32 = 320;
const HEIGHT: u32 = 60;

/// A pixel that blends RGBA colors so we can inspect the resulting color.
#[derive(Clone, Copy)]
struct RgbPixel {
    r: u8,
    g: u8,
    b: u8,
}

impl Default for RgbPixel {
    fn default() -> Self {
        RgbPixel { r: 0, g: 0, b: 0 }
    }
}

impl TargetPixel for RgbPixel {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let inv_alpha = 255u32 - color.alpha as u32;
        self.r = (color.red as u32 + self.r as u32 * inv_alpha / 255).min(255) as u8;
        self.g = (color.green as u32 + self.g as u32 * inv_alpha / 255).min(255) as u8;
        self.b = (color.blue as u32 + self.b as u32 * inv_alpha / 255).min(255) as u8;
    }


    fn lerp_from(&mut self, src: Self, mask: u8) {
        let m = mask as u32;
        let n = 255 - m;
        self.r = ((self.r as u32 * n + src.r as u32 * m) / 255) as u8;
        self.g = ((self.g as u32 * n + src.g as u32 * m) / 255) as u8;
        self.b = ((self.b as u32 * n + src.b as u32 * m) / 255) as u8;
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        RgbPixel { r, g, b }
    }
}

fn render(window: &Rc<MinimalSoftwareWindow>) -> Vec<RgbPixel> {
    let mut buf = vec![RgbPixel::default(); (WIDTH * HEIGHT) as usize];
    window.request_redraw();
    window.draw_if_needed(|renderer| {
        renderer.render(buf.as_mut_slice(), WIDTH as usize);
    });
    buf
}

/// White text on the explicit black window background shows up as bright
/// pixels; an explicit black background keeps a style/theme switch from
/// changing what "ink" means mid-test.
fn leftmost_glyph_col(buf: &[RgbPixel]) -> Option<u32> {
    let width = WIDTH as usize;
    let height = HEIGHT as usize;
    for col in 0..width {
        for row in 0..height {
            let p = buf[row * width + col];
            if p.r > 55 || p.g > 55 || p.b > 55 {
                return Some(col as u32);
            }
        }
    }
    None
}

/// Returns the rightmost column that contains a bright pixel.
fn rightmost_glyph_col(buf: &[RgbPixel]) -> Option<u32> {
    let width = WIDTH as usize;
    let height = HEIGHT as usize;
    for col in (0..width).rev() {
        for row in 0..height {
            let p = buf[row * width + col];
            if p.r > 55 || p.g > 55 || p.b > 55 {
                return Some(col as u32);
            }
        }
    }
    None
}

#[test]
fn rendered_ink_matches_reported_width() {
    let window = common::setup(WIDTH, HEIGHT);

    slint::slint! {
        export component TestCase inherits Window {
            in property <string> family: "Noto Sans";
            in property <[FontVariation]> axes: [];
            background: black;
            t := Text {
                text: "Hamburgefonstiv";
                color: white;
                font-size: 24px;
                font-family: root.family;
                font-variation-settings: root.axes;
            }
            out property <length> reported_width: t.width;
        }
    }

    let ui = TestCase::new().unwrap();
    ui.show().unwrap();

    // `wdth` on Noto Sans (62.5..100) and `opsz` on Roboto Flex (8..144): both
    // change the drawn width, and layout must agree with what was painted.
    let mut extents: Vec<(&str, &str, f32, f32)> = Vec::new();
    for (family, tag, value) in [
        ("Noto Sans", "wdth", 62.5f32),
        ("Noto Sans", "wdth", 100.),
        ("Noto Sans", "wght", 100.),
        ("Noto Sans", "wght", 900.),
        ("Roboto Flex", "opsz", 8.),
        ("Roboto Flex", "opsz", 144.),
        ("Roboto Flex", "wdth", 25.),
        ("Roboto Flex", "wdth", 151.),
    ] {
        ui.set_family(family.into());
        ui.set_axes(
            Rc::new(slint::VecModel::from_slice(&[slint::language::FontVariation {
                tag: tag.into(),
                value,
            }]))
            .into(),
        );

        let buf = render(&window);
        let inked_start = leftmost_glyph_col(&buf)
            .unwrap_or_else(|| panic!("{family} {tag} {value}: text must be rendered"));
        let inked_end = rightmost_glyph_col(&buf)
            .unwrap_or_else(|| panic!("{family} {tag} {value}: text must be rendered"));
        let reported = ui.get_reported_width();

        // The reported width is the sum of advances; the rightmost glyph's ink
        // ends before its advance box (a few px at 24 px font), while
        // antialiasing can spill ~1 px past it. Both effects stay well under
        // 4 px here — a bigger gap means layout and render disagree.
        let rendered_extent = inked_end as f32 + 1.0 - inked_start as f32;
        assert!(
            (rendered_extent - reported).abs() <= 4.0,
            "{family} {tag} {value}: reported width {reported}px but ink spans \
             {rendered_extent}px (columns {inked_start}..={inked_end})"
        );
        assert!(inked_end - inked_start > 50, "{family} {tag} {value}: text too short?");
        extents.push((family, tag, value, rendered_extent));
    }

    // The width the layout reports must actually move with the axis, not just
    // match whatever was painted of a static instance: for every (family, tag)
    // probed at two values the inked extent must differ measurably.
    for (family, tag) in [
        ("Noto Sans", "wdth"),
        ("Noto Sans", "wght"),
        ("Roboto Flex", "opsz"),
        ("Roboto Flex", "wdth"),
    ] {
        let mut pair = extents
            .iter()
            .filter(|(f, t, _, _)| *f == family && *t == tag)
            .map(|(_, _, v, e)| (*v, *e));
        let (v1, e1) = pair.next().unwrap();
        let (v2, e2) = pair.next().unwrap();
        assert!(
            (e1 - e2).abs() > 1.0,
            "{family} {tag} {v1} vs {v2}: rendered width barely moved ({e1}px vs {e2}px)"
        );
    }
}
