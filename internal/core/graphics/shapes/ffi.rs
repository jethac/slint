// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `extern "C"` entry points backing the `slint::shapes` C++ API and the
//! `Shapes.*` builtin functions in generated C++ code.
//!
//! Argument and output buffers are `void*`: `slint::Shape` is defined in the
//! handwritten `slint_shape.h` header (not by cbindgen) so passing the concrete
//! type here would drag `Shape` into every generated internal header. All point
//! arrays are flat x/y pairs and all rounding arrays flat radius/smoothing
//! pairs for the same reason.

#![allow(unsafe_code)]

use super::{Shape, api};
use crate::api::LogicalPosition;
use crate::items::{CornerRounding, FillRule};
use crate::model::ModelRc;
use core::ffi::c_void;

unsafe fn flat_to_points(coords: *const f32, len: usize) -> ModelRc<LogicalPosition> {
    if coords.is_null() || len == 0 {
        return ModelRc::default();
    }
    let flat = unsafe { core::slice::from_raw_parts(coords, len) };
    let points: alloc::vec::Vec<LogicalPosition> =
        flat.as_chunks::<2>().0.iter().map(|c| LogicalPosition::new(c[0], c[1])).collect();
    ModelRc::from(points.as_slice())
}

unsafe fn flat_to_roundings(roundings: *const f32, len: usize) -> ModelRc<CornerRounding> {
    if roundings.is_null() || len == 0 {
        return ModelRc::default();
    }
    let flat = unsafe { core::slice::from_raw_parts(roundings, len) };
    let roundings: alloc::vec::Vec<CornerRounding> =
        flat.as_chunks::<2>().0.iter().map(|c| CornerRounding::new(c[0], c[1])).collect();
    ModelRc::from(roundings.as_slice())
}

#[unsafe(no_mangle)]
/// `Shapes.polygon(vertices, rounding)`; `coords` is a flat x/y pair array.
pub unsafe extern "C" fn slint_shapes_polygon(
    coords: *const f32,
    coord_count: usize,
    radius: f32,
    smoothing: f32,
    out: *mut c_void,
) {
    let vertices = unsafe { flat_to_points(coords, coord_count) };
    let shape = api::rounded_polygon_polygon(&vertices, CornerRounding::new(radius, smoothing));
    unsafe { core::ptr::write(out as *mut Shape, shape) };
}

#[unsafe(no_mangle)]
/// `Shapes.polygon-per-vertex(vertices, roundings)`; `roundings` is a flat
/// radius/smoothing pair array.
pub unsafe extern "C" fn slint_shapes_polygon_per_vertex(
    coords: *const f32,
    coord_count: usize,
    roundings: *const f32,
    rounding_count: usize,
    out: *mut c_void,
) {
    let vertices = unsafe { flat_to_points(coords, coord_count) };
    let roundings = unsafe { flat_to_roundings(roundings, rounding_count) };
    let shape = api::rounded_polygon_per_vertex(&vertices, &roundings);
    unsafe { core::ptr::write(out as *mut Shape, shape) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_shapes_regular_polygon(
    num_vertices: i32,
    radius: f32,
    smoothing: f32,
    out: *mut c_void,
) {
    let shape = if num_vertices >= 3 {
        api::regular_polygon(num_vertices as usize, CornerRounding::new(radius, smoothing))
    } else {
        Shape::default()
    };
    unsafe { core::ptr::write(out as *mut Shape, shape) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_shapes_regular_polygon_per_vertex(
    num_vertices: i32,
    roundings: *const f32,
    rounding_count: usize,
    out: *mut c_void,
) {
    let roundings = unsafe { flat_to_roundings(roundings, rounding_count) };
    let shape = if num_vertices >= 3 {
        api::regular_polygon_per_vertex(num_vertices as usize, &roundings)
    } else {
        Shape::default()
    };
    unsafe { core::ptr::write(out as *mut Shape, shape) };
}

#[unsafe(no_mangle)]
/// `Shapes.rectangle(width, height, roundings)` — `roundings` is a flat
/// radius/smoothing pair array that may be empty, a single entry or four
/// entries.
pub unsafe extern "C" fn slint_shapes_rectangle(
    width: f32,
    height: f32,
    roundings: *const f32,
    rounding_count: usize,
    out: *mut c_void,
) {
    let roundings = unsafe { flat_to_roundings(roundings, rounding_count) };
    let shape = api::rectangle_shape(width, height, &roundings);
    unsafe { core::ptr::write(out as *mut Shape, shape) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_shapes_circle(num_vertices: i32, out: *mut c_void) {
    let shape = api::circle_shape(num_vertices.max(0) as usize);
    unsafe { core::ptr::write(out as *mut Shape, shape) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_shapes_star(
    num_vertices_per_radius: i32,
    inner_radius: f32,
    radius: f32,
    smoothing: f32,
    inner_radius_r: f32,
    inner_smoothing: f32,
    out: *mut c_void,
) {
    let shape = if num_vertices_per_radius >= 2 {
        api::star_shape(
            num_vertices_per_radius as usize,
            inner_radius,
            CornerRounding::new(radius, smoothing),
            CornerRounding::new(inner_radius_r, inner_smoothing),
        )
    } else {
        Shape::default()
    };
    unsafe { core::ptr::write(out as *mut Shape, shape) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_shapes_pill(
    width: f32,
    height: f32,
    smoothing: f32,
    out: *mut c_void,
) {
    let shape = api::pill_shape(width, height, smoothing);
    unsafe { core::ptr::write(out as *mut Shape, shape) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_shapes_pill_star(
    num_vertices_per_radius: i32,
    width: f32,
    height: f32,
    inner_radius_ratio: f32,
    radius: f32,
    smoothing: f32,
    out: *mut c_void,
) {
    let shape = if num_vertices_per_radius >= 1 {
        api::pill_star_shape(
            num_vertices_per_radius as usize,
            width,
            height,
            inner_radius_ratio,
            CornerRounding::new(radius, smoothing),
        )
    } else {
        Shape::default()
    };
    unsafe { core::ptr::write(out as *mut Shape, shape) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_shapes_custom(
    coords: *const f32,
    coord_count: usize,
    roundings: *const f32,
    rounding_count: usize,
    repetitions: i32,
    center_x: f32,
    center_y: f32,
    mirror: bool,
    out: *mut c_void,
) {
    let vertices = unsafe { flat_to_points(coords, coord_count) };
    let roundings = unsafe { flat_to_roundings(roundings, rounding_count) };
    let shape = api::custom_shape(
        &vertices,
        &roundings,
        repetitions,
        crate::api::LogicalPosition::new(center_x, center_y),
        mirror,
    );
    unsafe { core::ptr::write(out as *mut Shape, shape) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_shape_normalized(shape: *const c_void, out: *mut c_void) {
    let shape = unsafe { &*(shape as *const Shape) };
    unsafe { core::ptr::write(out as *mut Shape, shape.normalized()) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_shape_rotated(shape: *const c_void, degrees: f32, out: *mut c_void) {
    let shape = unsafe { &*(shape as *const Shape) };
    unsafe { core::ptr::write(out as *mut Shape, shape.rotated(degrees)) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_shape_scaled(
    shape: *const c_void,
    scale_x: f32,
    scale_y: f32,
    out: *mut c_void,
) {
    let shape = unsafe { &*(shape as *const Shape) };
    unsafe { core::ptr::write(out as *mut Shape, shape.scaled(scale_x, scale_y)) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_shape_translated(
    shape: *const c_void,
    dx: f32,
    dy: f32,
    out: *mut c_void,
) {
    let shape = unsafe { &*(shape as *const Shape) };
    unsafe { core::ptr::write(out as *mut Shape, shape.translated(dx, dy)) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_shape_morph(
    from: *const c_void,
    to: *const c_void,
    progress: f32,
    out: *mut c_void,
) {
    // Shapes arriving over FFI are `repr(C)` POD: a C++-side modified copy may
    // carry an id that no longer keys its payload, so re-key on ingress and
    // let the morph lookup take the content verify path.
    let mut from = unsafe { (*(from as *const Shape)).clone() };
    let mut to = unsafe { (*(to as *const Shape)).clone() };
    from.unkey();
    to.unkey();
    unsafe { core::ptr::write(out as *mut Shape, from.morph(&to, progress)) };
}

#[unsafe(no_mangle)]
/// `Shapes.path(d, fill_rule)`. The fill rule is stored on the resulting shape;
/// it only affects how the path renders when the shape is used for filling —
/// the polygonalization ignores it.
pub unsafe extern "C" fn slint_shapes_path(
    d: &crate::SharedString,
    fill_rule: FillRule,
    out: *mut c_void,
) {
    let shape = Shape::from_svg_path_lossy(d.as_str(), fill_rule);
    unsafe { core::ptr::write(out as *mut Shape, shape) };
}

#[unsafe(no_mangle)]
/// Compares two shapes for equality, as `slint::Shape::operator==`.
pub unsafe extern "C" fn slint_shape_compare_equal(a: *const c_void, b: *const c_void) -> bool {
    let a = unsafe { &*(a as *const Shape) };
    let b = unsafe { &*(b as *const Shape) };
    a == b
}

#[unsafe(no_mangle)]
/// Serializes a shape to an SVG path string, used by `Shape::to_svg_path()`.
pub unsafe extern "C" fn slint_shape_to_svg_path(
    shape: *const c_void,
    out: &mut crate::SharedString,
) {
    let shape = unsafe { &*(shape as *const Shape) };
    *out = shape.to_svg_path().into();
}
