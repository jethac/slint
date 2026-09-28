// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore androidx

//! Port of `Shapes.kt` from androidx.graphics.shapes, plus the `customPolygon` /
//! `doRepeat` helpers from `MaterialShapes.kt` (pinned commit
//! `23327507f7fc7d5b19d65fec4b090f60c970079b`).

use super::ShapeError;
use super::corner::CornerRounding;
use super::rounded_polygon::RoundedPolygon;
use super::utils::{
    FLOAT_PI, Point, TWO_PI, direction_vector_angle, distance, interpolate, k_atan2, k_cos, k_max,
    k_min, k_sin, radial_to_cartesian,
};
use alloc::vec::Vec;

/// Creates a circle, or at least a reasonable facsimile. Rounded polygons are not
/// really circles, but are approximations, including this pseudo-circle created by
/// this function. Increasing `num_vertices` leads to a more circular shape.
///
/// Returns an error if `num_vertices` < 3.
pub fn circle(
    num_vertices: usize,
    radius: f32,
    center: Point,
) -> Result<RoundedPolygon, ShapeError> {
    if num_vertices < 3 {
        return Err(ShapeError::new("Circle must have at least three vertices"));
    }

    // Half of the angle between two adjacent vertices on the polygon
    let theta = FLOAT_PI / num_vertices as f32;
    // Radius of the underlying RoundedPolygon object given the desired radius of the
    // circle
    let polygon_radius = radius / k_cos(theta);
    RoundedPolygon::regular(
        num_vertices,
        polygon_radius,
        center,
        CornerRounding { radius, smoothing: 0. },
        None,
    )
}

/// Creates a rectangular shape with the given width/height around the given center.
/// Optional rounding parameters can be used to create a rounded rectangle instead.
///
/// `per_vertex_rounding`, if supplied, must have 4 elements.
pub fn rectangle(
    width: f32,
    height: f32,
    rounding: CornerRounding,
    per_vertex_rounding: Option<Vec<CornerRounding>>,
    center: Point,
) -> Result<RoundedPolygon, ShapeError> {
    let left = center.x - width / 2.;
    let top = center.y - height / 2.;
    let right = center.x + width / 2.;
    let bottom = center.y + height / 2.;
    let polygon_vertices = [right, bottom, left, bottom, left, top, right, top];
    RoundedPolygon::from_vertices(
        &polygon_vertices,
        rounding,
        per_vertex_rounding.as_deref(),
        Some(center),
    )
}

/// Creates a star polygon, which is like a regular polygon except every other vertex
/// is on either an inner or outer radius. The two radii specified must both be nonzero
/// and positive, and `inner_radius` must be less than `radius`.
///
/// * `num_vertices_per_radius`: the number of vertices along each of the two radii.
/// * `radius`: outer radius for the star shape.
/// * `inner_radius`: inner radius for the star shape.
/// * `rounding`: the [CornerRounding] properties of every vertex.
/// * `inner_rounding`: optional rounding parameters for the vertices on the inner
///   radius. If None, inner vertices will use `rounding` or `per_vertex_rounding`
///   instead.
/// * `per_vertex_rounding`: if supplied, must have `2 * num_vertices_per_radius`
///   elements.
pub fn star(
    num_vertices_per_radius: usize,
    radius: f32,
    inner_radius: f32,
    rounding: CornerRounding,
    inner_rounding: Option<CornerRounding>,
    per_vertex_rounding: Option<Vec<CornerRounding>>,
    center: Point,
) -> Result<RoundedPolygon, ShapeError> {
    if radius <= 0. || inner_radius <= 0. {
        return Err(ShapeError::new("Star radii must both be greater than 0"));
    }
    if inner_radius >= radius {
        return Err(ShapeError::new("innerRadius must be less than radius"));
    }

    let mut pv_rounding = per_vertex_rounding;
    // If no per-vertex rounding supplied and caller asked for inner rounding,
    // create per-vertex rounding list based on supplied outer/inner rounding parameters
    if pv_rounding.is_none() {
        pv_rounding = inner_rounding.map(|inner_rounding| {
            (0..num_vertices_per_radius).flat_map(|_| [rounding, inner_rounding]).collect()
        });
    }

    // Star polygon is just a polygon with all vertices supplied (where we generate
    // those vertices to be on the inner/outer radii)
    RoundedPolygon::from_vertices(
        &star_vertices_from_num_verts(
            num_vertices_per_radius,
            radius,
            inner_radius,
            center.x,
            center.y,
        ),
        rounding,
        pv_rounding.as_deref(),
        Some(center),
    )
}

fn star_vertices_from_num_verts(
    num_vertices_per_radius: usize,
    radius: f32,
    inner_radius: f32,
    center_x: f32,
    center_y: f32,
) -> Vec<f32> {
    let mut result = Vec::with_capacity(num_vertices_per_radius * 4);
    for i in 0..num_vertices_per_radius {
        let vertex =
            radial_to_cartesian(radius, FLOAT_PI / num_vertices_per_radius as f32 * 2. * i as f32);
        result.push(vertex.x + center_x);
        result.push(vertex.y + center_y);
        let vertex = radial_to_cartesian(
            inner_radius,
            FLOAT_PI / num_vertices_per_radius as f32 * (2. * i as f32 + 1.),
        );
        result.push(vertex.x + center_x);
        result.push(vertex.y + center_y);
    }
    result
}

/// A pill shape consists of a rectangle shape bounded by two semicircles at either of
/// the long ends of the rectangle.
///
/// `smoothing` is the amount by which the arc is "smoothed" by extending the curve
/// from the circular arc on each endcap to the edge between the endcaps. A value of 0
/// (no smoothing) indicates that the corner is rounded by only a circular arc.
///
/// Returns an error if `width` or `height` is <= 0.
pub fn pill(
    width: f32,
    height: f32,
    smoothing: f32,
    center: Point,
) -> Result<RoundedPolygon, ShapeError> {
    if !(width > 0. && height > 0.) {
        return Err(ShapeError::new("Pill shapes must have positive width and height"));
    }

    let w_half = width / 2.;
    let h_half = height / 2.;
    RoundedPolygon::from_vertices(
        &[
            w_half + center.x,
            h_half + center.y,
            -w_half + center.x,
            h_half + center.y,
            -w_half + center.x,
            -h_half + center.y,
            w_half + center.x,
            -h_half + center.y,
        ],
        CornerRounding { radius: k_min(w_half, h_half), smoothing },
        None,
        Some(center),
    )
}

/// A pillStar shape is like a [pill] except it has inner and outer radii along its
/// pill-shaped outline, just like a [star] has inner and outer radii along its
/// circular outline.
///
/// * `width`/`height`: the resulting shape's dimensions, must be positive.
/// * `num_vertices_per_radius`: the number of vertices along each of the two radii.
/// * `inner_radius_ratio`: inner radius ratio, in the (0, 1] range. A value of 1
///   produces a [pill]-like shape with more vertices.
/// * `rounding`: [CornerRounding] for every vertex.
/// * `inner_rounding`: optional rounding for the inner-radius vertices.
/// * `per_vertex_rounding`: optional rounding per vertex; must have
///   `2 * num_vertices_per_radius` elements if supplied.
/// * `vertex_spacing`: determines how the vertices on the circular ends are laid out
///   along the outline; 0 aligns inner vertices like the straight-edge vertices, 1
///   aligns the outer vertices; 0.5 (the Kotlin default) averages the two.
/// * `start_location`: a value from 0 to 1 determining how far along the perimeter the
///   underlying curves begin.
#[allow(clippy::too_many_arguments)]
pub fn pill_star(
    width: f32,
    height: f32,
    num_vertices_per_radius: usize,
    inner_radius_ratio: f32,
    rounding: CornerRounding,
    inner_rounding: Option<CornerRounding>,
    per_vertex_rounding: Option<Vec<CornerRounding>>,
    vertex_spacing: f32,
    start_location: f32,
    center: Point,
) -> Result<RoundedPolygon, ShapeError> {
    if !(width > 0. && height > 0.) {
        return Err(ShapeError::new("Pill shapes must have positive width and height"));
    }
    if !(inner_radius_ratio > 0. && inner_radius_ratio <= 1.) {
        return Err(ShapeError::new("innerRadius must be between 0 and 1"));
    }

    let mut pv_rounding = per_vertex_rounding;
    // If no per-vertex rounding supplied and caller asked for inner rounding,
    // create per-vertex rounding list based on supplied outer/inner rounding parameters
    if pv_rounding.is_none() {
        pv_rounding = inner_rounding.map(|inner_rounding| {
            (0..num_vertices_per_radius).flat_map(|_| [rounding, inner_rounding]).collect()
        });
    }

    RoundedPolygon::from_vertices(
        &pill_star_vertices_from_num_verts(
            num_vertices_per_radius,
            width,
            height,
            inner_radius_ratio,
            vertex_spacing,
            start_location,
            center.x,
            center.y,
        ),
        rounding,
        pv_rounding.as_deref(),
        Some(center),
    )
}

#[allow(clippy::too_many_arguments)]
fn pill_star_vertices_from_num_verts(
    num_vertices_per_radius: usize,
    width: f32,
    height: f32,
    inner_radius: f32,
    vertex_spacing: f32,
    start_location: f32,
    center_x: f32,
    center_y: f32,
) -> Vec<f32> {
    // The general approach here is to get the perimeter of the underlying pill outline,
    // then the t value for each vertex as we walk that perimeter. This tells us where
    // on the outline to place that vertex, then we figure out where to place the vertex
    // depending on which "section" it is in. The possible sections are the vertical edges
    // on the sides, the circular sections on all four corners, or the horizontal edges
    // on the top and bottom. Note that either the vertical or horizontal edges will be
    // of length zero (whichever dimension is smaller gets only circular curvature for the
    // pill shape).
    let endcap_radius = k_min(width, height);
    let v_seg_len = k_max(height - width, 0.);
    let h_seg_len = k_max(width - height, 0.);
    let v_seg_half = v_seg_len / 2.;
    let h_seg_half = h_seg_len / 2.;
    // vertexSpacing is used to position the vertices on the end caps. The caller has the choice
    // of spacing the inner (0) or outer (1) vertices like those along the edges, causing the
    // other vertices to be either further apart (0) or closer (1). The default is .5, which
    // averages things. The magnitude of the inner and rounding parameters may cause the caller
    // to want a different value.
    let circle_perimeter = TWO_PI * endcap_radius * interpolate(inner_radius, 1., vertex_spacing);
    // perimeter is circle perimeter plus horizontal and vertical sections of inner rectangle,
    // whether either (or even both) might be of length zero.
    let perimeter = 2. * h_seg_len + 2. * v_seg_len + circle_perimeter;

    // The sections array holds the t start values of that part of the outline. We use these to
    // determine which section a given vertex lies in, based on its t value, as well as where
    // in that section it lies.
    let mut sections = [0f32; 11];
    sections[0] = 0.;
    sections[1] = v_seg_len / 2.;
    sections[2] = sections[1] + circle_perimeter / 4.;
    sections[3] = sections[2] + h_seg_len;
    sections[4] = sections[3] + circle_perimeter / 4.;
    sections[5] = sections[4] + v_seg_len;
    sections[6] = sections[5] + circle_perimeter / 4.;
    sections[7] = sections[6] + h_seg_len;
    sections[8] = sections[7] + circle_perimeter / 4.;
    sections[9] = sections[8] + v_seg_len / 2.;
    sections[10] = perimeter;

    // "t" is the length along the entire pill outline for a given vertex. With vertices spaced
    // evenly along this contour, we can determine for any vertex where it should lie.
    let t_per_vertex = perimeter / (2. * num_vertices_per_radius as f32);
    // separate iteration for inner vs outer, unlike the other shapes, because
    // the vertices can lie in different quadrants so each needs their own calculation
    let mut inner = false;
    // Increment section index as we walk around the pill contour with our increasing t values
    let mut curr_sec_index = 0usize;
    // secStart/End are used to determine how far along a given vertex is in the section
    // in which it lands. They persist across loop iterations and are only updated
    // inside the while loop below — matching Kotlin, including its behavior of
    // retaining stale values when currSecIndex is reset without the loop running.
    let mut sec_start = 0.;
    let mut sec_end = sections[1];
    // t value is used to place each vertex. 0 is on the positive x axis,
    // moving into section 0 to begin with. startLocation, a value from 0 to 1, varies the location
    // anywhere on the perimeter of the shape
    let mut t = start_location * perimeter;
    // The list of vertices to be returned
    let mut result = Vec::with_capacity(num_vertices_per_radius * 4);
    let rect_br = Point { x: h_seg_half, y: v_seg_half };
    let rect_bl = Point { x: -h_seg_half, y: v_seg_half };
    let rect_tl = Point { x: -h_seg_half, y: -v_seg_half };
    let rect_tr = Point { x: h_seg_half, y: -v_seg_half };
    // Each iteration through this loop uses the next t value as we walk around the shape
    for _ in 0..num_vertices_per_radius * 2 {
        // t could start (and end) after 0; extra boundedT logic makes sure it does the right
        // thing when crossing the boundary past 0 again
        let bounded_t = t % perimeter;
        if bounded_t < sec_start {
            curr_sec_index = 0;
        }
        while bounded_t >= sections[(curr_sec_index + 1) % sections.len()] {
            curr_sec_index = (curr_sec_index + 1) % sections.len();
            sec_start = sections[curr_sec_index];
            sec_end = sections[(curr_sec_index + 1) % sections.len()];
        }

        // find t in section and its proportion of that section's total length
        let t_in_section = bounded_t - sec_start;
        let t_proportion = t_in_section / (sec_end - sec_start);

        // The vertex placement in a section varies depending on whether it is on one of the
        // semicircle endcaps or along one of the straight edges. For the endcaps, we use
        // tProportion to get the angle along that circular cap and add
        // the starting angle for that section. For the edges we use a straight linear calculation
        // given tProportion and the start/end t values for that edge.
        let curr_radius = if inner { endcap_radius * inner_radius } else { endcap_radius };
        let vertex: Point = match curr_sec_index {
            0 => Point { x: curr_radius, y: t_proportion * v_seg_half },
            1 => radial_to_cartesian(curr_radius, t_proportion * FLOAT_PI / 2.) + rect_br,
            2 => Point { x: h_seg_half - t_proportion * h_seg_len, y: curr_radius },
            3 => {
                radial_to_cartesian(curr_radius, FLOAT_PI / 2. + (t_proportion * FLOAT_PI / 2.))
                    + rect_bl
            }
            4 => Point { x: -curr_radius, y: v_seg_half - t_proportion * v_seg_len },
            5 => {
                radial_to_cartesian(curr_radius, FLOAT_PI + (t_proportion * FLOAT_PI / 2.))
                    + rect_tl
            }
            6 => Point { x: -h_seg_half + t_proportion * h_seg_len, y: -curr_radius },
            7 => {
                radial_to_cartesian(curr_radius, FLOAT_PI * 1.5 + (t_proportion * FLOAT_PI / 2.))
                    + rect_tr
            }
            // 8
            _ => Point { x: curr_radius, y: -v_seg_half + t_proportion * v_seg_half },
        };
        result.push(vertex.x + center_x);
        result.push(vertex.y + center_y);
        t += t_per_vertex;
        inner = !inner;
    }
    result
}

/// Creates a polygon with the given vertices repeated `reps` times around `center`,
/// rotating each instance `360/reps` degrees; with `mirroring` each additional instance
/// is mirrored instead of rotated. Port of `customPolygon` + `doRepeat` from
/// MaterialShapes.kt.
///
/// `vertices` are the points of the base section as `(x, y)` pairs; `roundings`
/// supplies the [CornerRounding] per vertex.
///
/// Returns an error if `reps` is 0, or `roundings.len() != vertices.len()`.
pub fn custom_polygon(
    vertices: &[(f32, f32)],
    roundings: &[CornerRounding],
    center: Point,
    reps: usize,
    mirroring: bool,
) -> Result<RoundedPolygon, ShapeError> {
    if reps == 0 {
        return Err(ShapeError::new("reps must be >= 1"));
    }
    if roundings.len() != vertices.len() {
        return Err(ShapeError::new("roundings must have the same size as vertices"));
    }
    let (vertices2, pvr) = do_repeat(vertices, roundings, center, reps, mirroring);
    RoundedPolygon::from_vertices(&vertices2, CornerRounding::UNROUNDED, Some(&pvr), Some(center))
}

/// Port of `doRepeat` from MaterialShapes.kt: takes the input vertices + roundings and
/// repeats (or repeats mirrored) them `reps` times around `center`.
fn do_repeat(
    vertices: &[(f32, f32)],
    roundings: &[CornerRounding],
    center: Point,
    reps: usize,
    mirroring: bool,
) -> (Vec<f32>, Vec<CornerRounding>) {
    let mut new_vertices: Vec<f32> = Vec::new();
    let mut new_roundings: Vec<CornerRounding> = Vec::new();
    if mirroring {
        // Mirroring is done by duplicating the section and flipping every other one.
        let actual_reps = reps * 2;
        let section_angle = 360. / actual_reps as f32;
        let np = vertices.len();
        let angles: Vec<f32> =
            vertices.iter().map(|o| angle_degrees(o.0 - center.x, o.1 - center.y)).collect();
        let distances: Vec<f32> =
            vertices.iter().map(|o| distance(o.0 - center.x, o.1 - center.y)).collect();
        for rep in 0..actual_reps {
            for index in 0..np {
                let i = if rep % 2 == 0 { index } else { np - 1 - index };
                if !(rep % 2 == 1 && i == 0) {
                    let a = section_angle * rep as f32
                        + if rep % 2 == 0 {
                            angles[i]
                        } else {
                            section_angle - angles[i] + 2. * angles[0]
                        };
                    let a = to_radians(a);
                    let final_point = Point { x: k_cos(a), y: k_sin(a) } * distances[i] + center;
                    new_vertices.push(final_point.x);
                    new_vertices.push(final_point.y);
                    new_roundings.push(roundings[i]);
                }
            }
        }
    } else {
        for rep in 0..reps {
            for index in 0..vertices.len() {
                let v = rotate_degrees(
                    Point { x: vertices[index].0, y: vertices[index].1 },
                    rep as f32 * 360. / reps as f32,
                    center,
                );
                new_vertices.push(v.x);
                new_vertices.push(v.y);
                new_roundings.push(roundings[index]);
            }
        }
    }
    (new_vertices, new_roundings)
}

/// `Offset.rotateDegrees`: rotate `o` around `center` by `degrees`.
fn rotate_degrees(o: Point, degrees: f32, center: Point) -> Point {
    let off = o - center;
    let r = direction_vector_angle(to_radians(degrees));
    Point { x: r.x * off.x - r.y * off.y, y: r.y * off.x + r.x * off.y } + center
}

/// `toRadians()`: this/360f * 2*PI.toFloat() (all f32 ops).
pub(crate) fn to_radians(degrees: f32) -> f32 {
    degrees / 360. * 2. * FLOAT_PI
}

/// `Offset.angleDegrees()`: atan2(y, x) * 180f / PI.toFloat().
pub(crate) fn angle_degrees(x: f32, y: f32) -> f32 {
    k_atan2(y, x) * 180. / FLOAT_PI
}
