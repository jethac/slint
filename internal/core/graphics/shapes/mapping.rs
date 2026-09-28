// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore androidx

//! Port of `FloatMapping.kt` and `FeatureMapping.kt` from androidx.graphics.shapes.

use super::feature::Feature;
use super::utils::{DISTANCE_EPSILON, Point, distance_squared, k_min, positive_modulo};
use alloc::collections::BTreeSet;
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
pub(crate) fn linear_map(x_values: &[f32], y_values: &[f32], x: f32) -> f32 {
    assert!((0. ..=1.).contains(&x), "Invalid progress");
    let segment_start_index = x_values
        .iter()
        .enumerate()
        .find_map(|(i, _)| {
            let from = x_values[i];
            let to = x_values[(i + 1) % x_values.len()];
            if progress_in_range(x, from, to) { Some(i) } else { None }
        })
        .expect("No segment found for progress");
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
    positive_modulo(y_values[segment_start_index] + segment_size_y * position_in_segment, 1.)
}

/// Distance between two progress values. Since progress wraps around, a difference of
/// 0.99 counts as a distance of 0.01.
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
    /// A mapper from the given `(source, target)` progress pairs.
    ///
    /// Panics if the progress values are not valid (each in [0, 1), no repeats, at
    /// most one wrap) — matching Kotlin's `require`s.
    pub fn new(mappings: &[(f32, f32)]) -> DoubleMapper {
        let source_values: Vec<f32> = mappings.iter().map(|m| m.0).collect();
        let target_values: Vec<f32> = mappings.iter().map(|m| m.1).collect();
        validate_progress(&source_values);
        validate_progress(&target_values);
        DoubleMapper { source_values, target_values }
    }

    /// Maps `x` from source space to target space.
    pub fn map(&self, x: f32) -> f32 {
        linear_map(&self.source_values, &self.target_values, x)
    }

    /// Maps `x` back from target space to source space.
    pub fn map_back(&self, x: f32) -> f32 {
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
) -> DoubleMapper {
    // We only use corners for this mapping.
    let filtered_features1: Vec<usize> = (0..features1.len())
        .filter(|&i| matches!(features1[i].feature(), Feature::Corner { .. }))
        .collect();
    let filtered_features2: Vec<usize> = (0..features2.len())
        .filter(|&i| matches!(features2[i].feature(), Feature::Corner { .. }))
        .collect();

    let feature_progress_mapping = do_mapping(
        &filtered_features1.iter().map(|&i| features1[i].clone()).collect::<Vec<_>>(),
        &filtered_features2.iter().map(|&i| features2[i].clone()).collect::<Vec<_>>(),
    );

    DoubleMapper::new(&feature_progress_mapping)
}

struct DistanceVertex {
    distance: f32,
    f1: usize,
    f2: usize,
}

/// Returns a mapping of the features between `features1` and `features2`, sorted by
/// the progress of the first feature. Port of `doMapping`.
fn do_mapping(
    features1: &[ProgressableFeature],
    features2: &[ProgressableFeature],
) -> Vec<(f32, f32)> {
    let mut distance_vertex_list: Vec<DistanceVertex> = Vec::new();
    for (i1, f1) in features1.iter().enumerate() {
        for (i2, f2) in features2.iter().enumerate() {
            let d = feature_dist_squared(f1.feature(), f2.feature());
            if d != f32::MAX {
                distance_vertex_list.push(DistanceVertex { distance: d, f1: i1, f2: i2 });
            }
        }
    }
    distance_vertex_list.sort_by(|a, b| a.distance.total_cmp(&b.distance));

    // Special cases.
    if distance_vertex_list.is_empty() {
        return alloc::vec![(0., 0.), (0.5, 0.5)];
    }
    if distance_vertex_list.len() == 1 {
        let it = &distance_vertex_list[0];
        let f1 = features1[it.f1].progress();
        let f2 = features2[it.f2].progress();
        return alloc::vec![(f1, f2), ((f1 + 0.5) % 1., (f2 + 0.5) % 1.),];
    }

    let mut helper = MappingHelper::default();
    for dv in &distance_vertex_list {
        helper.add_mapping(&features1[dv.f1], dv.f1, &features2[dv.f2], dv.f2);
    }
    helper.mapping
}

#[derive(Default)]
struct MappingHelper {
    // List of mappings from progress in the start shape to progress in the end
    // shape. We keep this list sorted by the first element.
    mapping: Vec<(f32, f32)>,

    // Which features in the start shape have we used and which in the end shape.
    // Kotlin uses Set<ProgressableFeature> with reference equality on the Feature;
    // the index into the filtered lists plays that role here.
    used_f1: BTreeSet<usize>,
    used_f2: BTreeSet<usize>,
}

impl MappingHelper {
    fn add_mapping(
        &mut self,
        f1: &ProgressableFeature,
        f1_index: usize,
        f2: &ProgressableFeature,
        f2_index: usize,
    ) {
        // We don't want to map the same feature twice.
        if self.used_f1.contains(&f1_index) || self.used_f2.contains(&f2_index) {
            return;
        }

        // Ret is sorted, find where we need to insert this new mapping.
        let index = self.mapping.binary_search_by(|p| p.0.total_cmp(&f1.progress()));
        assert!(index.is_err(), "There can't be two features with the same progress");

        let insertion_index = index.unwrap_err();
        let n = self.mapping.len();

        // We can always add the first 1 element
        if n >= 1 {
            let (before1, before2) = self.mapping[(insertion_index + n - 1) % n];
            let (after1, after2) = self.mapping[insertion_index % n];

            // We don't want features that are way too close to each other, that will
            // make the DoubleMapper unstable
            if progress_distance(f1.progress(), before1) < DISTANCE_EPSILON
                || progress_distance(f1.progress(), after1) < DISTANCE_EPSILON
                || progress_distance(f2.progress(), before2) < DISTANCE_EPSILON
                || progress_distance(f2.progress(), after2) < DISTANCE_EPSILON
            {
                return;
            }

            // When we have 2 or more elements, we need to ensure we are not adding
            // extra crossings.
            if n > 1 && !progress_in_range(f2.progress(), before2, after2) {
                return;
            }
        }

        // All good, we can add the mapping.
        self.mapping.insert(insertion_index, (f1.progress(), f2.progress()));
        self.used_f1.insert(f1_index);
        self.used_f2.insert(f2_index);
    }
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
        _ => distance_squared(
            feature_representative_point(f1).x - feature_representative_point(f2).x,
            feature_representative_point(f1).y - feature_representative_point(f2).y,
        ),
    }
}

fn feature_representative_point(feature: &Feature) -> Point {
    let cubics = feature.cubics();
    let x = (cubics.first().unwrap().anchor0_x() + cubics.last().unwrap().anchor1_x()) / 2.;
    let y = (cubics.first().unwrap().anchor0_y() + cubics.last().unwrap().anchor1_y()) / 2.;
    Point { x, y }
}

/// Verify that a list of progress values are all in the range [0.0, 1.0) and is
/// monotonically increasing, with the exception of maybe one time in which the
/// progress wraps around. This check includes all pairs of consecutive elements in
/// the list plus the last-to-first element pair.
pub(crate) fn validate_progress(p: &[f32]) {
    let mut prev = *p.last().unwrap();
    let mut wraps = 0;
    for &curr in p {
        assert!(curr >= 0. && curr < 1., "FloatMapping - Progress outside of range");
        assert!(
            progress_distance(curr, prev) > DISTANCE_EPSILON,
            "FloatMapping - Progress repeats a value"
        );
        if curr < prev {
            wraps += 1;
            assert!(wraps <= 1, "FloatMapping - Progress wraps more than once");
        }
        prev = curr;
    }
}
