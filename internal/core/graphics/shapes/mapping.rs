// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore androidx

//! Port of `FloatMapping.kt` and `FeatureMapping.kt` from androidx.graphics.shapes.

use super::feature::Feature;
use super::utils::{Point, distance_squared, k_min, positive_modulo};
use alloc::vec::Vec;

/// Checks if the given progress is in the given progress range. Since progress is in
/// the [0, 1) interval and wraps, there is a special case when `progress_to <
/// progress_from`. For example, if the progress range is 0.7 to 0.2, both 0.8 and 0.1
/// are inside and 0.5 is outside.
pub(crate) fn progress_in_range(progress: f32, progress_from: f32, progress_to: f32) -> bool {
    if progress_to >= progress_from {
        progress >= progress_from && progress <= progress_to
    } else {
        progress >= progress_from || progress <= progress_to
    }
}

/// Maps from one set of progress values to another. This is used by [DoubleMapper] to
/// retrieve the value on one shape that maps to the appropriate value on the other.
///
/// Returns `None` for a progress outside [0, 1] or when no segment contains it
/// (the Kotlin `require`s, surfaced as a degenerate morph).
pub(crate) fn linear_map(x_values: &[f32], y_values: &[f32], x: f32) -> Option<f32> {
    if !(0. ..=1.).contains(&x) {
        return None;
    }
    let segment_start_index = x_values.iter().enumerate().find_map(|(i, _)| {
        let from = x_values[i];
        let to = x_values[(i + 1) % x_values.len()];
        if progress_in_range(x, from, to) { Some(i) } else { None }
    })?;
    let segment_end_index = (segment_start_index + 1) % x_values.len();
    let segment_size_x =
        positive_modulo(x_values[segment_end_index] - x_values[segment_start_index], 1.);
    let segment_size_y =
        positive_modulo(y_values[segment_end_index] - y_values[segment_start_index], 1.);
    let position_in_segment = if segment_size_x < 0.001 {
        0.5
    } else {
        positive_modulo(x - x_values[segment_start_index], 1.) / segment_size_x
    };
    Some(positive_modulo(y_values[segment_start_index] + segment_size_y * position_in_segment, 1.))
}

/// Distance between two progress values. Since progress wraps around, a difference of
/// 0.99 counts as a distance of 0.01.
#[allow(dead_code)]
pub(crate) fn progress_distance(p1: f32, p2: f32) -> f32 {
    let d = (p1 - p2).abs();
    k_min(d, 1. - d)
}

/// Creates mappings from values in the [0, 1) source space to values in the [0, 1)
/// target space, and back. The mapping is created from a finite list of
/// representative mappings, extended to the whole interval by linear interpolation
/// and wrapping around.
///
/// Port of `DoubleMapper`.
#[derive(Clone, Debug)]
pub struct DoubleMapper {
    source_values: Vec<f32>,
    target_values: Vec<f32>,
}

impl DoubleMapper {
    /// A mapper from the given `(source, target)` progress pairs, or `None` if the
    /// progress values are not valid (each in [0, 1), no repeats, at most one wrap) —
    /// matching Kotlin's `require`s.
    pub fn new(mappings: &[(f32, f32)]) -> Option<DoubleMapper> {
        let source_values: Vec<f32> = mappings.iter().map(|m| m.0).collect();
        let target_values: Vec<f32> = mappings.iter().map(|m| m.1).collect();
        if !validate_progress(&source_values) || !validate_progress(&target_values) {
            return None;
        }
        Some(DoubleMapper { source_values, target_values })
    }

    /// Maps `x` from source space to target space, or `None` for an invalid `x`.
    pub fn map(&self, x: f32) -> Option<f32> {
        linear_map(&self.source_values, &self.target_values, x)
    }

    /// Maps `x` back from target space to source space, or `None` for an invalid `x`.
    pub fn map_back(&self, x: f32) -> Option<f32> {
        linear_map(&self.target_values, &self.source_values, x)
    }
}

/// A [Feature] along with the progress at which it lies on a polygon's outline.
/// Port of `ProgressableFeature`.
#[derive(Clone, Debug)]
pub struct ProgressableFeature {
    progress: f32,
    feature: Feature,
}

impl ProgressableFeature {
    /// A feature at the given progress.
    pub fn new(progress: f32, feature: Feature) -> Self {
        Self { progress, feature }
    }

    /// The progress along the polygon's outline, in [0, 1).
    pub fn progress(&self) -> f32 {
        self.progress
    }

    /// The feature.
    pub fn feature(&self) -> &Feature {
        &self.feature
    }
}

/// Creates a mapping between the "features" (rounded corners) of two shapes.
/// Port of `featureMapper`.
pub(crate) fn feature_mapper(
    features1: &[ProgressableFeature],
    features2: &[ProgressableFeature],
) -> Option<DoubleMapper> {
    // We only use corners for this mapping.
    let filtered_features1: Vec<ProgressableFeature> = features1
        .iter()
        .filter(|f| matches!(f.feature(), Feature::Corner { .. }))
        .cloned()
        .collect();
    let filtered_features2: Vec<ProgressableFeature> = features2
        .iter()
        .filter(|f| matches!(f.feature(), Feature::Corner { .. }))
        .cloned()
        .collect();

    // doMapping maps the smaller feature list into the larger one.
    let (m1, m2) = if filtered_features1.len() > filtered_features2.len() {
        (do_mapping(&filtered_features2, &filtered_features1)?, filtered_features2)
    } else {
        let m2 = do_mapping(&filtered_features1, &filtered_features2)?;
        (filtered_features1, m2)
    };

    let mm: Vec<(f32, f32)> =
        m1.iter().zip(m2.iter()).map(|(a, b)| (a.progress(), b.progress())).collect();

    DoubleMapper::new(&mm)
}

/// Returns a mapping of the features in `features2` that best map to the features
/// in `features1`: a list of `features2` entries the size of `features1`. This is
/// done to figure out what the best features are in `features2` that map to the
/// existing features in `features1`. For example, if `features1` has 3 features
/// and `features2` has 4, we want to know what the 3 features are in `features2`
/// that map to the features in `features1` (then the morph will create a
/// placeholder feature in the smaller shape). Port of `doMapping`.
pub(crate) fn do_mapping(
    features1: &[ProgressableFeature],
    features2: &[ProgressableFeature],
) -> Option<Vec<ProgressableFeature>> {
    let m = features1.len();
    let n = features2.len();
    if m == 0 || n == 0 {
        return None;
    }

    // Pick the first mapping in a greedy way.
    // Kotlin's `minBy` returns the first minimal element (or throws on empty);
    // mirror that by tracking the best index by hand.
    let mut ix = 0usize;
    let mut ix_d = feature_dist_squared(features1[0].feature(), features2[0].feature());
    for (i2, f2) in features2.iter().enumerate().skip(1) {
        let d = feature_dist_squared(features1[0].feature(), f2.feature());
        if d < ix_d {
            ix_d = d;
            ix = i2;
        }
    }

    let mut ret = alloc::vec![features2[ix].clone()];
    let mut last_picked = ix as i64;
    for (i, f1) in features1.iter().enumerate().skip(1) {
        // Check the indices we can pick, which one is better.
        // Leave enough items in features2 to pick matches for the items left in features1.
        let last = {
            let raw = ix as i64 - (m as i64 - i as i64);
            if raw > last_picked { raw } else { raw + n as i64 }
        };
        // Kotlin's `minBy` still returns the first index when every candidate's
        // distance is `f32::MAX` (convex-vs-concave corners); track `INFINITY`
        // rather than `MAX` so those candidates are still picked.
        let mut best: Option<i64> = None;
        let mut best_d = f32::INFINITY;
        for cand in (last_picked + 1)..=last {
            let d =
                feature_dist_squared(f1.feature(), features2[(cand % n as i64) as usize].feature());
            if d < best_d {
                best_d = d;
                best = Some(cand);
            }
        }
        let best = best?;
        ret.push(features2[(best % n as i64) as usize].clone());
        last_picked = best;
    }
    Some(ret)
}

/// Distance along the overall shape between two Features on the two different shapes.
/// This information is used to determine how to map features (and the curves that
/// make up those features).
pub(crate) fn feature_dist_squared(f1: &Feature, f2: &Feature) -> f32 {
    match (f1, f2) {
        (Feature::Corner { convex: convex1, .. }, Feature::Corner { convex: convex2, .. })
            if convex1 != convex2 =>
        {
            // Simple hack to force all features to map only to features of the same
            // concavity, by returning an infinitely large distance in that case
            f32::MAX
        }
        _ => {
            let Some(p1) = feature_representative_point(f1) else { return f32::MAX };
            let Some(p2) = feature_representative_point(f2) else { return f32::MAX };
            distance_squared(p1.x - p2.x, p1.y - p2.y)
        }
    }
}

fn feature_representative_point(feature: &Feature) -> Option<Point> {
    let cubics = feature.cubics();
    let (first, last) = cubics.first().zip(cubics.last())?;
    Some(Point {
        x: (first.anchor0_x() + last.anchor1_x()) / 2.,
        y: (first.anchor0_y() + last.anchor1_y()) / 2.,
    })
}

/// Verify that a list of progress values are all in the range [0.0, 1.0] and is
/// monotonically increasing, with the exception of maybe one time in which the
/// progress wraps around.
pub(crate) fn validate_progress(p: &[f32]) -> bool {
    // FloatMapping - Progress outside of range
    if !p.iter().all(|&curr| (0. ..=1.).contains(&curr)) {
        return false;
    }
    // FloatMapping - Progress wraps more than once
    let wraps = (1..p.len()).filter(|&i| p[i] < p[i - 1]).count();
    wraps <= 1
}
