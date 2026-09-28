// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore androidx ish

//! Port of `Features.kt` and `FeatureDetector.kt` from androidx.graphics.shapes.

use super::ShapeError;
use super::cubic::Cubic;
use super::utils::{DISTANCE_EPSILON, PointTransformer, RELAXED_DISTANCE_EPSILON, collinear_ish};
use alloc::vec::Vec;

/// While a polygon's shape can be drawn solely using a list of [Cubic] objects
/// representing its raw curves and lines, features add an extra layer of context to
/// groups of cubics. Features group cubics into (straight) edges, convex corners, or
/// concave corners. For example, rounding a rectangle adds many cubics around its
/// edges, but the rectangle's overall number of corners remains the same. [Morph]
/// therefore uses this grouping for several reasons:
///
/// - Noise Reduction: Grouping cubics reduces the amount of noise introduced by
///   individual cubics (as seen in the rounded rectangle example).
/// - Mapping Base: The grouping serves as the base set for [Morph]'s mapping process.
/// - Curve Type Mapping: [Morph] maps similar curve types (convex, concave) together.
///   Note that edges or features created with [Feature::build_ignorable_feature] are
///   ignored in the default mapping.
///
/// [Morph]: crate::graphics::shapes::Morph
#[derive(Clone, Debug, PartialEq)]
pub enum Feature {
    /// Edges have only a list of the cubic curves which make up the edge. Edges lie
    /// between corners and have no vertex or concavity; the curves are simply straight
    /// lines (represented by cubic curves).
    Edge(Vec<Cubic>),
    /// Corners contain the list of cubic curves which describe how the corner is
    /// rounded (or not), and a flag indicating whether the corner is convex. A regular
    /// polygon has all convex corners, while a star polygon generally (but not
    /// necessarily) has both convex (outer) and concave (inner) corners.
    Corner {
        /// The cubic curves describing the corner's shape.
        cubics: Vec<Cubic>,
        /// Whether the corner is convex (outward indentation) or concave (inward).
        convex: bool,
    },
}

impl Feature {
    /// Group a list of [Cubic] objects to a feature that should be ignored in the
    /// default [Morph](crate::graphics::shapes::Morph) mapping. The feature can have
    /// any indentation.
    ///
    /// Only the features marked as important are smoothly transitioned between the
    /// start and end shapes of a morph.
    pub fn build_ignorable_feature(cubics: Vec<Cubic>) -> Result<Feature, ShapeError> {
        validated(Feature::Edge(cubics))
    }

    /// Group a [Cubic] object to an edge (neither inward nor outward identification in
    /// a shape).
    pub fn build_edge(cubic: Cubic) -> Result<Feature, ShapeError> {
        validated(Feature::Edge(alloc::vec![cubic]))
    }

    /// Group a list of [Cubic] objects to a convex corner (outward indentation in a
    /// shape).
    pub fn build_convex_corner(cubics: Vec<Cubic>) -> Result<Feature, ShapeError> {
        validated(Feature::Corner { cubics, convex: true })
    }

    /// Group a list of [Cubic] objects to a concave corner (inward indentation in a
    /// shape).
    pub fn build_concave_corner(cubics: Vec<Cubic>) -> Result<Feature, ShapeError> {
        validated(Feature::Corner { cubics, convex: false })
    }

    /// The raw cubics that make up this feature.
    pub fn cubics(&self) -> &[Cubic] {
        match self {
            Feature::Edge(cubics) => cubics,
            Feature::Corner { cubics, .. } => cubics,
        }
    }

    /// A new [Feature] with the points transformed by the given transformer.
    pub fn transformed(&self, f: &dyn PointTransformer) -> Feature {
        match self {
            Feature::Edge(cubics) => {
                Feature::Edge(cubics.iter().map(|c| c.transformed(f)).collect())
            }
            Feature::Corner { cubics, convex } => Feature::Corner {
                cubics: cubics.iter().map(|c| c.transformed(f)).collect(),
                convex: *convex,
            },
        }
    }

    /// A new [Feature] with the points that define the shape of this feature in
    /// reversed order.
    pub fn reversed(&self) -> Feature {
        match self {
            Feature::Edge(cubics) => {
                Feature::Edge(cubics.iter().rev().map(|c| c.reverse()).collect())
            }
            Feature::Corner { cubics, convex } => {
                // TODO: b/369320447 - Revert flag negation when [RoundedPolygon]
                // ignores orientation for setting the flag
                Feature::Corner {
                    cubics: cubics.iter().rev().map(|c| c.reverse()).collect(),
                    convex: !convex,
                }
            }
        }
    }

    /// Whether this feature gets ignored in the [Morph](crate::graphics::shapes::Morph)
    /// mapping. See [Feature::build_ignorable_feature] for details.
    pub fn is_ignorable_feature(&self) -> bool {
        matches!(self, Feature::Edge(..))
    }

    /// Whether this feature is an edge with no inward or outward indentation.
    pub fn is_edge(&self) -> bool {
        matches!(self, Feature::Edge(..))
    }

    /// Whether this feature is a convex corner (outward indentation in a shape).
    pub fn is_convex_corner(&self) -> bool {
        matches!(self, Feature::Corner { convex: true, .. })
    }

    /// Whether this feature is a concave corner (inward indentation in a shape).
    pub fn is_concave_corner(&self) -> bool {
        matches!(self, Feature::Corner { convex: false, .. })
    }
}

fn validated(feature: Feature) -> Result<Feature, ShapeError> {
    if feature.cubics().is_empty() {
        return Err(ShapeError::new("Features need at least one cubic."));
    }

    if !is_continuous(&feature) {
        return Err(ShapeError::new(
            "Feature must be continuous, with the anchor points of all cubics \
             matching the anchor points of the preceding and succeeding cubics",
        ));
    }

    Ok(feature)
}

fn is_continuous(feature: &Feature) -> bool {
    let cubics = feature.cubics();
    let mut prev_cubic = cubics[0];
    for cubic in &cubics[1..] {
        if (cubic.anchor0_x() - prev_cubic.anchor1_x()).abs() > DISTANCE_EPSILON
            || (cubic.anchor0_y() - prev_cubic.anchor1_y()).abs() > DISTANCE_EPSILON
        {
            return false;
        }
        prev_cubic = *cubic;
    }
    true
}

/// Convert cubics to features in a 1:1 mapping of `Cubic::as_feature` unless
/// - two subsequent cubics are not continuous, in which case an empty corner needs to
///   be added in between. Example for C1, C2: /C1\/C2\ -> /C\C/C\.
/// - multiple subsequent cubics can be expressed as a single feature. Example for C1,
///   C2: --C1----C2-- -> -----E----. One exception to the latter rule is for the first
///   and last cubic, that remain the same in order to persist the start position.
///
/// Assumes the list of cubics is continuous.
pub(crate) fn detect_features(cubics: &[Cubic]) -> Vec<Feature> {
    if cubics.is_empty() {
        return Vec::new();
    }

    // TODO: b/372651969 Try different heuristics for corner grouping
    let mut result = Vec::new();
    let mut current = cubics[0];

    // Do one roundabout in which (current == last, next == first) is the last
    // iteration. Just like a snowball, subsequent cubics that align to one feature
    // merge until the streak breaks, the result is added, and a new streak starts.
    for i in 0..cubics.len() {
        let next = cubics[(i + 1) % cubics.len()];

        if i < cubics.len() - 1 && current.aligns_ish_with(&next) {
            current = extend(current, next);
            continue;
        }

        result.push(current.as_feature(&next));

        if !current.smoothes_into_ish(&next) {
            result.push(Cubic::empty(current.anchor1_x(), current.anchor1_y()).as_feature(&next));
        }

        current = next;
    }
    result
}

impl Cubic {
    /// Convert to [Feature::Edge] if this cubic describes a straight line, otherwise
    /// to a [Feature::Corner]. Corner convexity is determined by `convex`.
    fn as_feature(&self, next: &Cubic) -> Feature {
        if self.straight_ish() {
            Feature::Edge(alloc::vec![*self])
        } else {
            Feature::Corner { cubics: alloc::vec![*self], convex: self.convex_to(next) }
        }
    }

    /// Determine if the cubic is close to a straight line. Empty cubics don't count as
    /// straight-ish.
    fn straight_ish(&self) -> bool {
        !self.zero_length()
            && collinear_ish(
                self.anchor0_x(),
                self.anchor0_y(),
                self.anchor1_x(),
                self.anchor1_y(),
                self.control0_x(),
                self.control0_y(),
                RELAXED_DISTANCE_EPSILON,
            )
            && collinear_ish(
                self.anchor0_x(),
                self.anchor0_y(),
                self.anchor1_x(),
                self.anchor1_y(),
                self.control1_x(),
                self.control1_y(),
                RELAXED_DISTANCE_EPSILON,
            )
    }

    /// Determine if next is a smooth continuation of this cubic. Smooth meaning that
    /// the first control point of next is a reflection of this' second control point,
    /// similar to the S/s or t/T command in svg paths.
    fn smoothes_into_ish(&self, next: &Cubic) -> bool {
        collinear_ish(
            self.control1_x(),
            self.control1_y(),
            next.control0_x(),
            next.control0_y(),
            self.anchor1_x(),
            self.anchor1_y(),
            RELAXED_DISTANCE_EPSILON,
        )
    }

    /// Determine if all of this' points align with next's points. For straight lines,
    /// this is the same as if next was a continuation of this.
    fn aligns_ish_with(&self, next: &Cubic) -> bool {
        (self.straight_ish() && next.straight_ish() && self.smoothes_into_ish(next))
            || self.zero_length()
            || next.zero_length()
    }
}

/// A new cubic extending `a` to `b`'s second anchor point.
fn extend(a: Cubic, b: Cubic) -> Cubic {
    if a.zero_length() {
        Cubic::from_floats(
            a.anchor0_x(),
            a.anchor0_y(),
            b.control0_x(),
            b.control0_y(),
            b.control1_x(),
            b.control1_y(),
            b.anchor1_x(),
            b.anchor1_y(),
        )
    } else {
        Cubic::from_floats(
            a.anchor0_x(),
            a.anchor0_y(),
            a.control0_x(),
            a.control0_y(),
            a.control1_x(),
            a.control1_y(),
            b.anchor1_x(),
            b.anchor1_y(),
        )
    }
}
