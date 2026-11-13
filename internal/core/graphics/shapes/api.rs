// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore androidx

//! The `Shapes.*` builtin functions exposed to the `.slint` language and the
//! `slint::shapes` Rust namespace. These are thin wrappers over the
//! androidx.graphics.shapes port: they take the language-level argument types
//! ([`crate::items::CornerRounding`], [`LogicalPosition`]) and return a
//! [`Shape`], mapping invalid arguments to the empty shape.

use super::shape::Shape;
use crate::items::CornerRounding;
use rounded_polygon::{Point, RoundedPolygon};
use crate::api::LogicalPosition;
use crate::model::{Model as _, ModelRc};
use alloc::vec::Vec;

impl CornerRounding {
    /// A rounding radius of zero, producing a sharp corner at a vertex.
    pub const UNROUNDED: CornerRounding = CornerRounding { radius: 0., smoothing: 0. };

    /// Creates a corner rounding from the `radius` of its circular arc and a `smoothing` factor from 0 to 1.
    pub const fn new(radius: f32, smoothing: f32) -> Self {
        Self { radius, smoothing }
    }
}

impl From<CornerRounding> for rounded_polygon::CornerRounding {
    fn from(rounding: CornerRounding) -> Self {
        Self { radius: rounding.radius, smoothing: rounding.smoothing }
    }
}

const UNROUNDED: rounded_polygon::CornerRounding = rounded_polygon::CornerRounding::UNROUNDED;

fn polygon_roundings(roundings: &ModelRc<CornerRounding>) -> Vec<rounded_polygon::CornerRounding> {
    roundings.iter().map(Into::into).collect()
}

fn shape(polygon: Result<RoundedPolygon, rounded_polygon::ShapeError>) -> Shape {
    polygon.map(|p| Shape::from_polygon(&p)).unwrap_or_else(|e| {
        crate::debug_log!("Shapes: {e}");
        Shape::default()
    })
}

/// `Shapes.polygon(vertices, rounding)`: a polygon over `vertices`, each corner
/// rounded with `rounding`.
pub fn rounded_polygon_polygon(
    vertices: &ModelRc<LogicalPosition>,
    rounding: CornerRounding,
) -> Shape {
    let flat: Vec<f32> = vertices.iter().flat_map(|p| [p.x, p.y]).collect();
    let rounding = rounding.into();
    let roundings = alloc::vec![rounding; vertices.row_count()];
    shape(RoundedPolygon::from_vertices(&flat, rounding, Some(&roundings), None))
}

/// `Shapes.polygon-per-vertex(vertices, roundings)`: a polygon whose each corner
/// takes its rounding from `roundings` (must match `vertices` in length).
pub fn rounded_polygon_per_vertex(
    vertices: &ModelRc<LogicalPosition>,
    roundings: &ModelRc<CornerRounding>,
) -> Shape {
    let roundings = polygon_roundings(roundings);
    if vertices.row_count() == 0 || vertices.row_count() != roundings.len() {
        return Shape::default();
    }
    let flat: Vec<f32> = vertices.iter().flat_map(|p| [p.x, p.y]).collect();
    shape(RoundedPolygon::from_vertices(&flat, UNROUNDED, Some(&roundings), None))
}

/// `Shapes.regular-polygon(num-vertices, rounding)`: a regular `num-vertices`-gon
/// with unit radius centered on the origin.
pub fn regular_polygon(num_vertices: usize, rounding: CornerRounding) -> Shape {
    shape(RoundedPolygon::regular(num_vertices, 1., Point::ZERO, rounding.into(), None))
}

/// `Shapes.regular-polygon-per-vertex(num-vertices, roundings)`.
pub fn regular_polygon_per_vertex(
    num_vertices: usize,
    roundings: &ModelRc<CornerRounding>,
) -> Shape {
    let roundings = polygon_roundings(roundings);
    if num_vertices != roundings.len() {
        return Shape::default();
    }
    shape(RoundedPolygon::regular(
        num_vertices,
        1.,
        Point::ZERO,
        UNROUNDED,
        Some(&roundings),
    ))
}

/// `Shapes.rectangle(width, height, roundings)`: a `width` × `height` rectangle
/// centered on the origin. `roundings` may be empty (sharp corners), a single
/// element (uniform rounding) or four elements (per-corner rounding).
pub fn rectangle_shape(width: f32, height: f32, roundings: &ModelRc<CornerRounding>) -> Shape {
    let roundings = polygon_roundings(roundings);
    let (rounding, per_vertex) = match roundings.len() {
        0 => (UNROUNDED, None),
        1 => (roundings[0], None),
        4 => (UNROUNDED, Some(roundings)),
        _ => return Shape::default(),
    };
    shape(rounded_polygon::rectangle(width, height, rounding, per_vertex, Point::ZERO))
}

/// `Shapes.circle(num-vertices)`: a unit pseudo-circle approximated with
/// `num-vertices` vertices, centered on the origin.
pub fn circle_shape(num_vertices: usize) -> Shape {
    shape(rounded_polygon::circle(num_vertices, 1., Point::ZERO))
}

/// `Shapes.star(num-vertices-per-radius, inner-radius, rounding, inner-rounding)`:
/// a unit star with `num-vertices-per-radius` outer and inner vertices.
pub fn star_shape(
    num_vertices_per_radius: usize,
    inner_radius: f32,
    rounding: CornerRounding,
    inner_rounding: CornerRounding,
) -> Shape {
    shape(rounded_polygon::star(
        num_vertices_per_radius,
        1.,
        inner_radius,
        rounding.into(),
        Some(inner_rounding.into()),
        None,
        Point::ZERO,
    ))
}

/// `Shapes.pill(width, height, smoothing)`.
pub fn pill_shape(width: f32, height: f32, smoothing: f32) -> Shape {
    shape(rounded_polygon::pill(width, height, smoothing, Point::ZERO))
}

/// `Shapes.pill-star(num-vertices-per-radius, width, height, inner-radius-ratio, rounding)`.
pub fn pill_star_shape(
    num_vertices_per_radius: usize,
    width: f32,
    height: f32,
    inner_radius_ratio: f32,
    rounding: CornerRounding,
) -> Shape {
    shape(rounded_polygon::pill_star(
        width,
        height,
        num_vertices_per_radius,
        inner_radius_ratio,
        rounding.into(),
        None,
        None,
        1.,
        0.5,
        Point::ZERO,
    ))
}

/// `Shapes.custom(vertices, roundings, reps, center, mirror)`: a polygon built
/// from `vertices` replicated `reps` times around `center`, optionally mirrored
/// — the `customPolygon`/`doRepeat` machinery from MaterialShapes.kt.
pub fn custom_shape(
    vertices: &ModelRc<LogicalPosition>,
    roundings: &ModelRc<CornerRounding>,
    reps: i32,
    center: LogicalPosition,
    mirror: bool,
) -> Shape {
    if reps <= 0 {
        return Shape::default();
    }
    let vertices: Vec<(f32, f32)> = vertices.iter().map(|p| (p.x, p.y)).collect();
    let roundings = polygon_roundings(roundings);
    shape(rounded_polygon::custom_polygon(
        &vertices,
        &roundings,
        Point { x: center.x, y: center.y },
        reps as usize,
        mirror,
    ))
}
