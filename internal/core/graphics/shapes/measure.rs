// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore androidx

//! Port of `PolygonMeasure.kt` from androidx.graphics.shapes.

use super::cubic::Cubic;
use super::feature::Feature;
use super::mapping::ProgressableFeature;
use super::rounded_polygon::RoundedPolygon;
use super::utils::{DISTANCE_EPSILON, Point, positive_modulo};
use alloc::rc::Rc;
use alloc::vec::Vec;

/// Describes the strategy for measuring distances along a polygon's outline.
/// Port of the `Measurer` interface.
pub trait Measurer {
    /// The measured size of `cubic` (for example, its arc length). Must be >= 0.
    fn measure_cubic(&self, cubic: &Cubic) -> f32;
    /// The progress `t` along `cubic` at which it should be cut so that the first
    /// part has measured size `m`. `m` should be between 0 and
    /// `measure_cubic(cubic)` (if not, it is capped).
    fn find_cubic_cut_point(&self, cubic: &Cubic, m: f32) -> f32;
}

/// Approximates the arc lengths of cubics by splitting the arc into `segments` linear
/// segments and calculating their sizes. The more segments, the more accurate the
/// result will be to the true arc length. The default of 3 segments achieves at least
/// 98.5% accuracy on a circular arc, which is the worst case for the standard shapes.
pub struct LengthMeasurer {
    segments: usize,
}

impl LengthMeasurer {
    /// A measurer with the given number of linear segments per cubic.
    pub fn new(segments: usize) -> Self {
        Self { segments }
    }
}

impl Default for LengthMeasurer {
    fn default() -> Self {
        Self { segments: 3 }
    }
}

impl Measurer for LengthMeasurer {
    fn measure_cubic(&self, cubic: &Cubic) -> f32 {
        self.closest_progress_to(cubic, f32::INFINITY).1
    }

    fn find_cubic_cut_point(&self, cubic: &Cubic, m: f32) -> f32 {
        self.closest_progress_to(cubic, m).0
    }
}

impl LengthMeasurer {
    /// Returns `(progress, measured_size)` — the progress value `t` along the cubic at
    /// which its measured length reaches `threshold`, and the total measured size.
    fn closest_progress_to(&self, cubic: &Cubic, threshold: f32) -> (f32, f32) {
        let mut total = 0f32;
        let mut remainder = threshold;
        let mut prev = Point { x: cubic.anchor0_x(), y: cubic.anchor0_y() };

        for i in 1..=self.segments {
            let progress = i as f32 / self.segments as f32;
            let point = cubic.point_on_curve(progress);
            let segment = (point - prev).distance();

            if segment >= remainder {
                return (progress - (1. - remainder / segment) / self.segments as f32, threshold);
            }

            remainder -= segment;
            total += segment;
            prev = point;
        }

        (1., total)
    }
}

/// A [Cubic] along with the progress values (start and end) it spans along its
/// polygon's measured outline. Port of `MeasuredCubic`.
///
/// Outline progress is a value in [0, 1) that represents the distance traveled along
/// the overall outline path of the shape.
#[derive(Clone, Debug)]
pub struct MeasuredCubic {
    /// The underlying cubic.
    pub cubic: Cubic,
    /// Progress along the polygon's outline where this cubic starts.
    pub start_outline_progress: f32,
    /// Progress along the polygon's outline where this cubic ends.
    pub end_outline_progress: f32,
    /// The measured size of this cubic (e.g. its approximate length).
    pub measured_size: f32,
}

impl MeasuredCubic {
    fn new(
        cubic: Cubic,
        measurer: &dyn Measurer,
        start_outline_progress: f32,
        end_outline_progress: f32,
    ) -> Self {
        let end_outline_progress = if end_outline_progress >= start_outline_progress {
            end_outline_progress
        } else {
            crate::debug_log!(
                "Shapes: endOutlineProgress is expected to be equal or greater than startOutlineProgress"
            );
            start_outline_progress
        };
        Self {
            cubic,
            start_outline_progress,
            end_outline_progress,
            measured_size: measurer.measure_cubic(&cubic),
        }
    }

    /// Cut this measured cubic into two at `cut_outline_progress` (in the polygon's
    /// progress space) and return the pair `(before, after)` the cut, or `None` if
    /// the measurer can't find a cut point on this cubic (a degenerate or non-finite
    /// outline, where Kotlin's `require`s throw).
    pub(crate) fn cut_at_progress(
        &self,
        measurer: &dyn Measurer,
        cut_outline_progress: f32,
    ) -> Option<(MeasuredCubic, MeasuredCubic)> {
        // Floating point errors further up can cause cutOutlineProgress to land just
        // slightly outside of the start/end progress for this cubic, so we limit it
        // to those bounds to avoid further errors later
        let bounded_cut_outline_progress =
            cut_outline_progress.clamp(self.start_outline_progress, self.end_outline_progress);
        let outline_progress_size = self.end_outline_progress - self.start_outline_progress;
        let progress_from_start = bounded_cut_outline_progress - self.start_outline_progress;

        // Note that in earlier parts of the computation, we have empty MeasuredCubics
        // (cubics with progressSize == 0f), but those cubics are filtered out before
        // this method is called.
        let relative_progress = progress_from_start / outline_progress_size;
        let t = measurer.find_cubic_cut_point(&self.cubic, relative_progress * self.measured_size);
        if !(0. ..=1.).contains(&t) {
            return None;
        }

        // c1/c2 are the two new cubics, then we return MeasuredCubics created from
        // them
        let (c1, c2) = self.cubic.split(t);
        Some((
            MeasuredCubic::new(
                c1,
                measurer,
                self.start_outline_progress,
                bounded_cut_outline_progress,
            ),
            MeasuredCubic::new(
                c2,
                measurer,
                bounded_cut_outline_progress,
                self.end_outline_progress,
            ),
        ))
    }

    fn update_progress_range(&mut self, start_outline_progress: f32, end_outline_progress: f32) {
        if end_outline_progress >= start_outline_progress {
            self.start_outline_progress = start_outline_progress;
            self.end_outline_progress = end_outline_progress;
        } else {
            crate::debug_log!(
                "Shapes: endOutlineProgress is expected to be equal or greater than startOutlineProgress"
            );
            self.start_outline_progress = start_outline_progress;
            self.end_outline_progress = start_outline_progress;
        }
    }
}

/// A [RoundedPolygon] whose cubics have been measured along its outline.
/// Port of `MeasuredPolygon`.
#[derive(Clone)]
pub struct MeasuredPolygon {
    measurer: Rc<dyn Measurer>,
    cubics: Vec<MeasuredCubic>,
    /// The polygon's corner features along with their progress along the outline.
    pub features: Vec<ProgressableFeature>,
}

impl MeasuredPolygon {
    /// The private Kotlin constructor: `cubics`/`outline_progress` describe the same
    /// outline; `outline_progress` must have `cubics.len() + 1` entries starting at 0
    /// and ending at 1.
    /// Returns `None` when `outline_progress` doesn't describe a complete
    /// outline (one more entry than `cubics`, starting at 0, ending at 1) —
    /// the Kotlin implementation rejects that with a `require`.
    fn new(
        measurer: Rc<dyn Measurer>,
        features: Vec<ProgressableFeature>,
        cubics: Vec<Cubic>,
        outline_progress: &[f32],
    ) -> Option<MeasuredPolygon> {
        if outline_progress.len() != cubics.len() + 1
            || outline_progress.first() != Some(&0.)
            || outline_progress.last() != Some(&1.)
        {
            crate::debug_log!(
                "Shapes: outline progress is expected to have one more entry than cubics, starting at 0 and ending at 1"
            );
            return None;
        }

        let mut measured_cubics = Vec::new();
        let mut start_outline_progress = 0f32;
        for index in 0..cubics.len() {
            // Filter out "empty" cubics
            if outline_progress[index + 1] - outline_progress[index] > DISTANCE_EPSILON {
                measured_cubics.push(MeasuredCubic::new(
                    cubics[index],
                    measurer.as_ref(),
                    start_outline_progress,
                    outline_progress[index + 1],
                ));
                // The next measured cubic will start exactly where this one ends.
                start_outline_progress = outline_progress[index + 1];
            }
        }
        // We could have removed empty cubics at the end. Ensure the last measured
        // cubic ends at 1f
        if let Some(last) = measured_cubics.last_mut() {
            let s = last.start_outline_progress;
            last.update_progress_range(s, 1.);
        }

        Some(MeasuredPolygon { measurer, cubics: measured_cubics, features })
    }

    /// The number of measured cubics.
    pub fn size(&self) -> usize {
        self.cubics.len()
    }

    /// The measured cubic at `index`, or None if out of bounds.
    pub fn get(&self, index: usize) -> Option<MeasuredCubic> {
        self.cubics.get(index).cloned()
    }

    /// The measurer used by this polygon.
    pub(crate) fn measurer(&self) -> &Rc<dyn Measurer> {
        &self.measurer
    }

    /// Finds the point in the input list of measured cubics that passes the given
    /// outline progress, and generates a new MeasuredPolygon (equivalent to this),
    /// that starts at that point. Port of `cutAndShift`.
    ///
    /// Returns `None` for a cutting point outside [0, 1] or a cubic that can't be
    /// cut there (the Kotlin `require`s, surfaced by [Morph](super::morph::Morph)
    /// as a degenerate morph).
    pub fn cut_and_shift(&self, cutting_point: f32) -> Option<MeasuredPolygon> {
        if !(0. ..=1.).contains(&cutting_point) {
            return None;
        }
        if cutting_point < DISTANCE_EPSILON {
            return Some(self.clone());
        }

        // Find the index of cubic we want to cut
        let target_index = self.cubics.iter().position(|mc| {
            cutting_point >= mc.start_outline_progress && cutting_point <= mc.end_outline_progress
        })?;
        let target = &self.cubics[target_index];

        // Cut the target cubic.
        // b1, b2 are two resulting cubics after cut
        let (b1, b2) = target.cut_at_progress(self.measurer.as_ref(), cutting_point)?;

        // Construct the list of the cubics we need:
        // * The second part of the target cubic (after the cut)
        // * All cubics after the target, until the end + All cubics from the start,
        //   before the target cubic
        // * The first part of the target cubic (before the cut)
        let mut ret_cubics = alloc::vec![b2.cubic];
        for i in 1..self.cubics.len() {
            ret_cubics.push(self.cubics[(i + target_index) % self.cubics.len()].cubic);
        }
        ret_cubics.push(b1.cubic);

        // Construct the array of outline progress.
        let mut ret_outline_progress = Vec::with_capacity(self.cubics.len() + 2);
        for index in 0..self.cubics.len() + 2 {
            ret_outline_progress.push(if index == 0 {
                0.
            } else if index == self.cubics.len() + 1 {
                1.
            } else {
                let cubic_index = (target_index + index - 1) % self.cubics.len();
                positive_modulo(self.cubics[cubic_index].end_outline_progress - cutting_point, 1.)
            });
        }

        // Shift the feature's outline progress too.
        let new_features: Vec<ProgressableFeature> = self
            .features
            .iter()
            .map(|pf| {
                ProgressableFeature::new(
                    positive_modulo(pf.progress() - cutting_point, 1.),
                    pf.feature().clone(),
                )
            })
            .collect();

        // Filter out all empty cubics (i.e. start and end anchor are (almost) the
        // same point.)
        MeasuredPolygon::new(
            Rc::clone(&self.measurer),
            new_features,
            ret_cubics,
            &ret_outline_progress,
        )
    }

    /// A [MeasuredPolygon] for `polygon`, measured with `measurer`.
    /// Port of `MeasuredPolygon.measurePolygon`.
    ///
    /// Returns `None` when the polygon's outline can't be measured: a cubic whose
    /// measured size is negative or non-finite, or a total outline size that is
    /// zero or non-finite (e.g. a zero-scaled shape or a NaN vertex). Kotlin's
    /// `require`s throw in those cases; callers here degrade to a defined result
    /// instead.
    pub fn measure_polygon(
        measurer: Rc<dyn Measurer>,
        polygon: &RoundedPolygon,
    ) -> Option<MeasuredPolygon> {
        let mut cubics = Vec::new();
        let mut feature_to_cubic: Vec<(Feature, usize)> = Vec::new();

        // Get the cubics from the polygon, at the same time, extract the features and
        // keep a reference to the representative cubic we will use.
        for feature in polygon.features() {
            for (cubic_index_in_feature, cubic) in feature.cubics().iter().enumerate() {
                if matches!(feature, Feature::Corner { .. })
                    && cubic_index_in_feature == feature.cubics().len() / 2
                {
                    feature_to_cubic.push((feature.clone(), cubics.len()));
                }
                cubics.push(*cubic);
            }
        }

        let mut measures = Vec::with_capacity(cubics.len() + 1);
        measures.push(0f32);
        for cubic in &cubics {
            let measured = measurer.measure_cubic(cubic);
            if !(measured >= 0. && measured.is_finite()) {
                return None;
            }
            measures.push(measures.last().unwrap() + measured);
        }
        let total_measure = *measures.last().unwrap();
        if !(total_measure > DISTANCE_EPSILON && total_measure.is_finite()) {
            // A zero-length outline produces NaN outline progress values, which the
            // Kotlin implementation rejects with a `require` in the MeasuredPolygon
            // constructor.
            return None;
        }

        let outline_progress: Vec<f32> = measures.iter().map(|m| m / total_measure).collect();

        let features: Vec<ProgressableFeature> = feature_to_cubic
            .iter()
            .map(|(f, ix)| {
                ProgressableFeature::new(
                    positive_modulo((outline_progress[*ix] + outline_progress[ix + 1]) / 2., 1.),
                    f.clone(),
                )
            })
            .collect();

        MeasuredPolygon::new(measurer, features, cubics, &outline_progress)
    }
}

/// `MeasuredPolygon.measurePolygon` convenience wrapper.
#[allow(dead_code)]
pub fn measure_polygon(
    measurer: Rc<dyn Measurer>,
    polygon: &RoundedPolygon,
) -> Option<MeasuredPolygon> {
    MeasuredPolygon::measure_polygon(measurer, polygon)
}
