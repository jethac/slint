// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore androidx

//! Port of `RoundedPolygon.kt` and `PolygonValidation.kt` from androidx.graphics.shapes.

use super::ShapeError;
use super::corner::{CornerRounding, RoundedCorner};
use super::cubic::Cubic;
use super::feature::Feature;
use super::utils::{
    DISTANCE_EPSILON, FLOAT_PI, Point, PointTransformer, convex, distance, distance_squared, k_max,
    k_min, k_sqrt, radial_to_cartesian,
};
use alloc::vec::Vec;

/// Allows simple construction of polygonal shapes with optional rounding at the
/// vertices. Polygons can be constructed with either the number of vertices desired or
/// an ordered list of vertices.
#[derive(Clone, Debug, PartialEq)]
pub struct RoundedPolygon {
    features: Vec<Feature>,
    center: Point,
    /// A flattened version of the features, as a `Vec<Cubic>`.
    cubics: Vec<Cubic>,
}

impl RoundedPolygon {
    /// A single-point polygon at `center`. Used where an invalid outline must be
    /// replaced by a defined result.
    pub(crate) fn degenerate(center: Point) -> Self {
        Self { features: Vec::new(), center, cubics: alloc::vec![Cubic::empty(center.x, center.y)] }
    }

    /// Creates a polygon from its [Feature] list and center. The [Feature]s describe
    /// the characteristics of each outline segment of the polygon.
    ///
    /// The `center` is optional; if not supplied, it is estimated by calculating the
    /// average of all cubic anchor points.
    ///
    /// The contiguity of the outline is validated, like the Kotlin constructor's
    /// `init` block does.
    pub fn from_features(
        features: Vec<Feature>,
        center: Option<Point>,
    ) -> Result<RoundedPolygon, ShapeError> {
        if features.len() < 2 {
            return Err(ShapeError::new("Polygons must have at least 2 features"));
        }

        let mut vertices = Vec::new();
        for feature in &features {
            for cubic in feature.cubics() {
                vertices.push(cubic.anchor0_x());
                vertices.push(cubic.anchor0_y());
            }
        }

        let c = match center {
            Some(center) => center,
            None => calculate_center(&vertices),
        };

        Self::from_features_center_unchecked(features, c)
    }

    /// The Kotlin internal constructor `RoundedPolygon(features, center)`: computes
    /// `cubics` eagerly and validates outline contiguity.
    pub(crate) fn from_features_center_unchecked(
        features: Vec<Feature>,
        center: Point,
    ) -> Result<RoundedPolygon, ShapeError> {
        // The first/last mechanism here ensures that the final anchor point in the
        // shape exactly matches the first anchor point. There can be rendering
        // artifacts introduced by those points being slightly off, even by much less
        // than a pixel
        let mut cubics = Vec::new();
        let mut first_cubic: Option<Cubic> = None;
        let mut last_cubic: Option<Cubic> = None;
        let mut first_feature_split_start: Option<Vec<Cubic>> = None;
        let mut first_feature_split_end: Option<Vec<Cubic>> = None;
        if !features.is_empty() && features[0].cubics().len() == 3 {
            let center_cubic = features[0].cubics()[1];
            let (start, end) = center_cubic.split(0.5);
            first_feature_split_start = Some(alloc::vec![features[0].cubics()[0], start]);
            first_feature_split_end = Some(alloc::vec![end, features[0].cubics()[2]]);
        }
        // iterating one past the features list size allows us to insert the initial
        // split cubic if it exists
        for i in 0..=features.len() {
            let feature_cubics: &[Cubic] = if i == 0 && first_feature_split_end.is_some() {
                first_feature_split_end.as_deref().unwrap()
            } else if i == features.len() {
                match &first_feature_split_start {
                    Some(list) => list,
                    None => break,
                }
            } else {
                features[i].cubics()
            };
            for cubic in feature_cubics {
                // Skip zero-length curves; they add nothing and can trigger rendering
                // artifacts
                if !cubic.zero_length() {
                    if let Some(last) = last_cubic {
                        cubics.push(last);
                    }
                    last_cubic = Some(*cubic);
                    if first_cubic.is_none() {
                        first_cubic = Some(*cubic);
                    }
                } else {
                    if let Some(mut last) = last_cubic {
                        // Dropping several zero-ish length curves in a row can lead to
                        // enough discontinuity to throw an exception later, even though
                        // the distances are quite small. Account for that by making the
                        // last cubic use the latest anchor point, always.
                        last.points[6] = cubic.anchor1_x();
                        last.points[7] = cubic.anchor1_y();
                        last_cubic = Some(last);
                    }
                }
            }
        }
        match (last_cubic, first_cubic) {
            (Some(last), Some(first)) => {
                cubics.push(Cubic::from_floats(
                    last.anchor0_x(),
                    last.anchor0_y(),
                    last.control0_x(),
                    last.control0_y(),
                    last.control1_x(),
                    last.control1_y(),
                    first.anchor0_x(),
                    first.anchor0_y(),
                ));
            }
            _ => {
                // Empty / 0-sized polygon.
                cubics.push(Cubic::from_floats(
                    center.x, center.y, center.x, center.y, center.x, center.y, center.x, center.y,
                ));
            }
        }

        // Contiguity validation, matching the Kotlin constructor's init block.
        let mut prev_cubic = cubics[cubics.len() - 1];
        for cubic in &cubics {
            if (cubic.anchor0_x() - prev_cubic.anchor1_x()).abs() > DISTANCE_EPSILON
                || (cubic.anchor0_y() - prev_cubic.anchor1_y()).abs() > DISTANCE_EPSILON
            {
                return Err(ShapeError::new(
                    "RoundedPolygon must be contiguous, with the anchor points of all curves \
                     matching the anchor points of the preceding and succeeding cubics",
                ));
            }
            prev_cubic = *cubic;
        }

        Ok(RoundedPolygon { features, center, cubics })
    }

    /// The features of this polygon.
    pub fn features(&self) -> &[Feature] {
        &self.features
    }

    /// The flattened list of cubics of this polygon.
    pub fn cubics(&self) -> &[Cubic] {
        &self.cubics
    }

    /// The center x coordinate.
    pub fn center_x(&self) -> f32 {
        self.center.x
    }

    /// The center y coordinate.
    pub fn center_y(&self) -> f32 {
        self.center.y
    }

    /// The center point.
    pub fn center(&self) -> Point {
        self.center
    }

    /// Transforms (scales/translates/etc.) this polygon with the given
    /// [PointTransformer] and returns a new [RoundedPolygon]. This is a low-level API;
    /// more idiomatic ways to transform a polygon are provided as methods on the
    /// `shape` values.
    ///
    /// Transforming cannot introduce discontinuities for a well-behaved
    /// transformer, so the constructor's contiguity requirement is still satisfied;
    /// the check is kept, matching the Kotlin `init`/`require` behavior, and
    /// surfaces as an error for a transform that does break it.
    pub fn transformed(&self, f: &dyn PointTransformer) -> Result<RoundedPolygon, ShapeError> {
        let center = self.center.transformed(f);
        RoundedPolygon::from_features_center_unchecked(
            self.features.iter().map(|feat| feat.transformed(f)).collect(),
            center,
        )
    }

    /// A new [RoundedPolygon], moved and resized so it is completely inside the
    /// (0, 0) -> (1, 1) square, centered if there is extra space in one direction.
    ///
    /// Returns an error for a zero-sized polygon (side of 0 divides the
    /// coordinates into non-finite values).
    pub fn normalized(&self) -> Result<RoundedPolygon, ShapeError> {
        let bounds = self.calculate_bounds(true);
        let width = bounds[2] - bounds[0];
        let height = bounds[3] - bounds[1];
        let side = k_max(width, height);
        // `>` (not `<=`-rejection) matches Kotlin's `require`, which also fails on NaN.
        if !matches!(side.partial_cmp(&DISTANCE_EPSILON), Some(core::cmp::Ordering::Greater)) {
            return Err(ShapeError::new("Can't normalize a zero-sized shape"));
        }
        // Center the shape if bounds are not a square
        let offset_x = (side - width) / 2. - bounds[0]; /* left */
        let offset_y = (side - height) / 2. - bounds[1]; /* top */
        self.transformed(&|x: f32, y: f32| Point {
            x: (x + offset_x) / side,
            y: (y + offset_y) / side,
        })
    }

    /// Like [RoundedPolygon::calculate_bounds], this function calculates the
    /// axis-aligned bounds of the object and returns that rectangle. But this function
    /// determines the max dimension of the shape (by calculating the distance from its
    /// center to the start and midpoint of each curve) and returns a square which can
    /// be used to hold the object in any rotation.
    ///
    /// Returns the axis-aligned max bounding box `[left, top, right, bottom]`.
    pub fn calculate_max_bounds(&self) -> [f32; 4] {
        let mut max_dist_squared = 0f32;
        for cubic in &self.cubics {
            let anchor_distance = distance_squared(
                cubic.anchor0_x() - self.center.x,
                cubic.anchor0_y() - self.center.y,
            );
            let middle_point = cubic.point_on_curve(0.5);
            let middle_distance =
                distance_squared(middle_point.x - self.center.x, middle_point.y - self.center.y);
            max_dist_squared = k_max(max_dist_squared, k_max(anchor_distance, middle_distance));
        }
        let distance = k_sqrt(max_dist_squared);
        [
            self.center.x - distance,
            self.center.y - distance,
            self.center.x + distance,
            self.center.y + distance,
        ]
    }

    /// Calculates the axis-aligned bounds of the object.
    ///
    /// `approximate`: when true, uses a faster calculation to create the bounding box
    /// based on the min/max values of all anchor and control points that make up the
    /// shape.
    ///
    /// Returns the axis-aligned bounding box `[left, top, right, bottom]`.
    pub fn calculate_bounds(&self, approximate: bool) -> [f32; 4] {
        // Kotlin's Float.MIN_VALUE is the smallest positive (subnormal) float, which is
        // f32::from_bits(1) — not the most negative float — so this matches the Kotlin
        // behavior exactly, including its quirk for fully-negative shapes.
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::from_bits(1);
        let mut max_y = f32::from_bits(1);
        for cubic in &self.cubics {
            let bounds = cubic.calculate_bounds(approximate);
            min_x = k_min(min_x, bounds[0]);
            min_y = k_min(min_y, bounds[1]);
            max_x = k_max(max_x, bounds[2]);
            max_y = k_max(max_y, bounds[3]);
        }
        [min_x, min_y, max_x, max_y]
    }

    /// Creates a polygon from the number of vertices in the resulting polygon. These
    /// vertices are positioned on a virtual circle around a given center with each
    /// vertex positioned `radius` distance from that center, equally spaced (with equal
    /// angles between them). With the default radius of 1 the shape's vertices lie on a
    /// unit circle, with width/height of 2.
    ///
    /// The `rounding` and `per_vertex_rounding` parameters are optional. If not
    /// supplied, the result is a regular polygon with straight edges and unrounded
    /// corners. `per_vertex_rounding`, if given, must have `num_vertices` elements.
    ///
    /// Returns an error if `num_vertices` < 3 or `per_vertex_rounding` has the wrong
    /// size.
    pub fn regular(
        num_vertices: usize,
        radius: f32,
        center: Point,
        rounding: CornerRounding,
        per_vertex_rounding: Option<&[CornerRounding]>,
    ) -> Result<RoundedPolygon, ShapeError> {
        Self::from_vertices(
            &vertices_from_num_verts(num_vertices, radius, center.x, center.y),
            rounding,
            per_vertex_rounding,
            Some(center),
        )
    }

    /// Takes the vertices (either supplied or calculated, depending on the constructor
    /// called), plus [CornerRounding] parameters, and creates the actual
    /// [RoundedPolygon] shape, rounding around the vertices (or not) as specified. The
    /// result is a list of [Cubic] curves which represent the geometry of the final
    /// shape.
    ///
    /// `vertices` is an ordered list of x/y coordinate pairs (the outline of the shape
    /// goes from each vertex to the next in order of this list), otherwise the results
    /// are undefined.
    ///
    /// Returns an error if there are less than 3 vertices (`vertices` has less than 6
    /// floats), if `vertices` has odd size, or if `per_vertex_rounding` is not None and
    /// its size doesn't match the number of vertices.
    pub fn from_vertices(
        vertices: &[f32],
        rounding: CornerRounding,
        per_vertex_rounding: Option<&[CornerRounding]>,
        center: Option<Point>,
    ) -> Result<RoundedPolygon, ShapeError> {
        if vertices.len() < 6 {
            return Err(ShapeError::new("Polygons must have at least 3 vertices"));
        }
        if vertices.len() % 2 == 1 {
            return Err(ShapeError::new("The vertices array should have even size"));
        }
        if per_vertex_rounding.is_some_and(|pvr| pvr.len() * 2 != vertices.len()) {
            return Err(ShapeError::new(
                "perVertexRounding list should be either empty or \
                 the same size as the number of vertices (vertices.len / 2)",
            ));
        }
        let n = vertices.len() / 2;
        let mut rounded_corners: Vec<RoundedCorner> = Vec::with_capacity(n);
        for i in 0..n {
            let vtx_rounding = per_vertex_rounding.map(|pvr| pvr[i]).unwrap_or(rounding);
            let prev_index = ((i + n - 1) % n) * 2;
            let next_index = ((i + 1) % n) * 2;
            rounded_corners.push(RoundedCorner::new(
                Point { x: vertices[prev_index], y: vertices[prev_index + 1] },
                Point { x: vertices[i * 2], y: vertices[i * 2 + 1] },
                Point { x: vertices[next_index], y: vertices[next_index + 1] },
                Some(vtx_rounding),
            ));
        }

        // For each side, check if we have enough space to do the cuts needed, and if
        // not split the available space, first for round cuts, then for smoothing if
        // there is space left. Each element in this list is a pair, that represents
        // how much we can do of the cut for the given side (side i goes from corner i
        // to corner i+1), the elements of the pair are: first is how much we can use
        // of expectedRoundCut, second how much of expectedCut
        let cut_adjusts: Vec<(f32, f32)> = (0..n)
            .map(|ix| {
                let expected_round_cut = rounded_corners[ix].expected_round_cut()
                    + rounded_corners[(ix + 1) % n].expected_round_cut();
                let expected_cut = rounded_corners[ix].expected_cut()
                    + rounded_corners[(ix + 1) % n].expected_cut();
                let vtx_x = vertices[ix * 2];
                let vtx_y = vertices[ix * 2 + 1];
                let next_vtx_x = vertices[((ix + 1) % n) * 2];
                let next_vtx_y = vertices[((ix + 1) % n) * 2 + 1];
                let side_size = distance(vtx_x - next_vtx_x, vtx_y - next_vtx_y);

                // Check expectedRoundCut first, and ensure we fulfill rounding needs
                // first for both corners before using space for smoothing
                if expected_round_cut > side_size {
                    // Not enough room for fully rounding, see how much we can actually
                    // do.
                    (side_size / expected_round_cut, 0.)
                } else if expected_cut > side_size {
                    // We can do full rounding, but not full smoothing.
                    (1., (side_size - expected_round_cut) / (expected_cut - expected_round_cut))
                } else {
                    // There is enough room for rounding & smoothing.
                    (1., 1.)
                }
            })
            .collect();
        // Create and store list of beziers for each [potentially] rounded corner
        let mut corners: Vec<Vec<Cubic>> = Vec::with_capacity(n);
        for (i, rounded_corner) in rounded_corners.iter_mut().enumerate() {
            // allowed_cuts[0] is for the side from the previous corner to this one,
            // allowed_cuts[1] is for the side from this corner to the next one.
            let mut allowed_cuts = [0.; 2];
            for delta in 0..=1 {
                let (round_cut_ratio, cut_ratio) = cut_adjusts[(i + n - 1 + delta) % n];
                allowed_cuts[delta] = rounded_corner.expected_round_cut() * round_cut_ratio
                    + (rounded_corner.expected_cut() - rounded_corner.expected_round_cut())
                        * cut_ratio;
            }
            let corner_cubics =
                rounded_corner.cubics(allowed_cuts[0], allowed_cuts[1]).ok_or_else(|| {
                    ShapeError::new(
                        "Can't compute the rounded corner on a degenerate polygon vertex",
                    )
                })?;
            corners.push(corner_cubics);
        }
        // Finally, store the calculated cubics. This includes all of the rounded
        // corners from above, along with new cubics representing the edges between
        // those corners.
        let mut temp_features: Vec<Feature> = Vec::new();
        for i in 0..n {
            // Note that these indices are for pairs of values (points), they need to
            // be doubled to access the xy values in the vertices float array
            let prev_vtx_index = (i + n - 1) % n;
            let next_vtx_index = (i + 1) % n;
            let curr_vertex = Point { x: vertices[i * 2], y: vertices[i * 2 + 1] };
            let prev_vertex =
                Point { x: vertices[prev_vtx_index * 2], y: vertices[prev_vtx_index * 2 + 1] };
            let next_vertex =
                Point { x: vertices[next_vtx_index * 2], y: vertices[next_vtx_index * 2 + 1] };
            let is_convex = convex(prev_vertex, curr_vertex, next_vertex);
            temp_features.push(Feature::Corner { cubics: corners[i].clone(), convex: is_convex });
            temp_features.push(Feature::Edge(alloc::vec![Cubic::straight_line(
                corners[i].last().unwrap().anchor1_x(),
                corners[i].last().unwrap().anchor1_y(),
                corners[(i + 1) % n].first().unwrap().anchor0_x(),
                corners[(i + 1) % n].first().unwrap().anchor0_y(),
            )]));
        }

        let c = match center {
            // Kotlin: centerX == Float.MIN_VALUE || centerY == Float.MIN_VALUE acts as
            // "no center provided"; the Rust API uses Option instead.
            Some(center) => center,
            None => calculate_center(vertices),
        };
        Self::from_features_center_unchecked(temp_features, c)
    }
}

/// Calculates an estimated center position for the polygon, returning it. This function
/// should only be called if the center is not already calculated or provided.
///
/// Note that this center is transformed whenever the shape itself is transformed. Any
/// transforms that occur before the center is calculated are taken into account
/// automatically since the center calculation is an average of the current location of
/// all cubic anchor points.
pub(crate) fn calculate_center(vertices: &[f32]) -> Point {
    let mut cumulative_x = 0.;
    let mut cumulative_y = 0.;
    let mut index = 0;
    while index < vertices.len() {
        cumulative_x += vertices[index];
        index += 1;
        cumulative_y += vertices[index];
        index += 1;
    }
    Point {
        x: cumulative_x / (vertices.len() / 2) as f32,
        y: cumulative_y / (vertices.len() / 2) as f32,
    }
}

fn vertices_from_num_verts(
    num_vertices: usize,
    radius: f32,
    center_x: f32,
    center_y: f32,
) -> Vec<f32> {
    let mut result = Vec::with_capacity(num_vertices * 2);
    for i in 0..num_vertices {
        let vertex = radial_to_cartesian(radius, FLOAT_PI / num_vertices as f32 * 2. * i as f32)
            + Point { x: center_x, y: center_y };
        result.push(vertex.x);
        result.push(vertex.y);
    }
    result
}

/// Validates whether this [RoundedPolygon]'s orientation is clockwise and fixes it if
/// necessary. Port of `PolygonValidator.fix`.
///
/// Correct input means: closed geometry, clockwise orientation of points, no
/// self-intersections, no holes, single polygon.
pub(crate) fn fix_polygon_orientation(
    polygon: &RoundedPolygon,
) -> Result<RoundedPolygon, ShapeError> {
    if is_cw_oriented(polygon) { Ok(polygon.clone()) } else { fix_cw_orientation(polygon) }
}

fn is_cw_oriented(polygon: &RoundedPolygon) -> bool {
    let mut signed_area = 0f32;
    for cubic in &polygon.cubics {
        signed_area +=
            (cubic.anchor1_x() - cubic.anchor0_x()) * (cubic.anchor1_y() + cubic.anchor0_y());
    }
    signed_area < 0.
}

fn fix_cw_orientation(polygon: &RoundedPolygon) -> Result<RoundedPolygon, ShapeError> {
    let mut reversed_features = Vec::with_capacity(polygon.features.len());
    // Persist first feature to stay a Corner
    reversed_features.push(polygon.features[0].reversed());
    for i in (1..polygon.features.len()).rev() {
        reversed_features.push(polygon.features[i].reversed());
    }
    // The center does not change with orientation flips.
    RoundedPolygon::from_features_center_unchecked(reversed_features, polygon.center)
}
