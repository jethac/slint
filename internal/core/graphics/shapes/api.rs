// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore androidx

//! The `Shapes.*` builtin functions exposed to the `.slint` language and the
//! `slint::shapes` Rust namespace. These are thin wrappers over the
//! androidx.graphics.shapes port: they take the language-level argument types
//! ([`crate::items::CornerRounding`], [`LogicalPosition`]) and return a
//! [`Shape`], mapping invalid arguments to the empty shape.

use super::corner::CornerRounding;
use super::rounded_polygon::RoundedPolygon;
use super::shape::Shape;
use super::utils::Point;
use crate::api::LogicalPosition;
use crate::model::{Model as _, ModelRc};
use alloc::vec::Vec;

fn shape(polygon: Result<RoundedPolygon, super::ShapeError>) -> Shape {
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
    let roundings = alloc::vec![rounding; vertices.row_count()];
    shape(RoundedPolygon::from_vertices(&flat, rounding, Some(&roundings), None))
}

/// `Shapes.polygon-per-vertex(vertices, roundings)`: a polygon whose each corner
/// takes its rounding from `roundings` (must match `vertices` in length).
pub fn rounded_polygon_per_vertex(
    vertices: &ModelRc<LogicalPosition>,
    roundings: &ModelRc<CornerRounding>,
) -> Shape {
    let roundings: Vec<CornerRounding> = roundings.iter().collect();
    if vertices.row_count() == 0 || vertices.row_count() != roundings.len() {
        return Shape::default();
    }
    let flat: Vec<f32> = vertices.iter().flat_map(|p| [p.x, p.y]).collect();
    shape(RoundedPolygon::from_vertices(&flat, CornerRounding::UNROUNDED, Some(&roundings), None))
}

/// `Shapes.regular-polygon(num-vertices, rounding)`: a regular `num-vertices`-gon
/// with unit radius centered on the origin.
pub fn regular_polygon(num_vertices: usize, rounding: CornerRounding) -> Shape {
    shape(RoundedPolygon::regular(num_vertices, 1., Point::ZERO, rounding, None))
}

/// `Shapes.regular-polygon-per-vertex(num-vertices, roundings)`.
pub fn regular_polygon_per_vertex(
    num_vertices: usize,
    roundings: &ModelRc<CornerRounding>,
) -> Shape {
    let roundings: Vec<CornerRounding> = roundings.iter().collect();
    if num_vertices != roundings.len() {
        return Shape::default();
    }
    shape(RoundedPolygon::regular(
        num_vertices,
        1.,
        Point::ZERO,
        CornerRounding::UNROUNDED,
        Some(&roundings),
    ))
}

/// `Shapes.rectangle(width, height, roundings)`: a `width` × `height` rectangle
/// centered on the origin. `roundings` may be empty (sharp corners), a single
/// element (uniform rounding) or four elements (per-corner rounding).
pub fn rectangle_shape(width: f32, height: f32, roundings: &ModelRc<CornerRounding>) -> Shape {
    let roundings: Vec<CornerRounding> = roundings.iter().collect();
    let (rounding, per_vertex) = match roundings.len() {
        0 => (CornerRounding::UNROUNDED, None),
        1 => (roundings[0], None),
        4 => (CornerRounding::UNROUNDED, Some(roundings)),
        _ => return Shape::default(),
    };
    shape(super::constructors::rectangle(width, height, rounding, per_vertex, Point::ZERO))
}

/// `Shapes.circle(num-vertices)`: a unit pseudo-circle approximated with
/// `num-vertices` vertices, centered on the origin.
pub fn circle_shape(num_vertices: usize) -> Shape {
    shape(super::constructors::circle(num_vertices, 1., Point::ZERO))
}

/// `Shapes.star(num-vertices-per-radius, inner-radius, rounding, inner-rounding)`:
/// a unit star with `num-vertices-per-radius` outer and inner vertices.
pub fn star_shape(
    num_vertices_per_radius: usize,
    inner_radius: f32,
    rounding: CornerRounding,
    inner_rounding: CornerRounding,
) -> Shape {
    shape(super::constructors::star(
        num_vertices_per_radius,
        1.,
        inner_radius,
        rounding,
        Some(inner_rounding),
        None,
        Point::ZERO,
    ))
}

/// `Shapes.pill(width, height, smoothing)`.
pub fn pill_shape(width: f32, height: f32, smoothing: f32) -> Shape {
    shape(super::constructors::pill(width, height, smoothing, Point::ZERO))
}

/// `Shapes.pill-star(num-vertices-per-radius, width, height, inner-radius-ratio, rounding)`.
pub fn pill_star_shape(
    num_vertices_per_radius: usize,
    width: f32,
    height: f32,
    inner_radius_ratio: f32,
    rounding: CornerRounding,
) -> Shape {
    shape(super::constructors::pill_star(
        width,
        height,
        num_vertices_per_radius,
        inner_radius_ratio,
        rounding,
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
    let roundings: Vec<CornerRounding> = roundings.iter().collect();
    shape(super::constructors::custom_polygon(
        &vertices,
        &roundings,
        Point { x: center.x, y: center.y },
        reps as usize,
        mirror,
    ))
}
