// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::dynamic_color::DynamicColor;

/// Documents a constraint in tone distance between two DynamicColors.
///
/// The polarity is an adjective that describes "A", compared to "B".
#[derive(Clone)]
pub struct ToneDeltaPair {
    /// The first role in a pair.
    pub role_a: DynamicColor,
    /// The second role in a pair.
    pub role_b: DynamicColor,
    /// Required difference between tones in absolute value.
    pub delta: f64,
    /// The relative relation between tones of roleA and roleB, as described above.
    pub polarity: TonePolarity,
    /// Whether the two roles should stay on the same side of the "awkward zone"
    /// (T50-59) in the case when the "awkward zone" is skipped.
    pub stay_together: bool,
    /// How to fulfill the tone delta pair.
    pub constraint: DeltaConstraint,
}

impl ToneDeltaPair {
    pub fn new(
        role_a: DynamicColor,
        role_b: DynamicColor,
        delta: f64,
        polarity: TonePolarity,
        stay_together: bool,
        constraint: DeltaConstraint,
    ) -> Self {
        Self { role_a, role_b, delta, polarity, stay_together, constraint }
    }
}

/// Describes how to fulfill a tone delta pair constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaConstraint {
    /// The tone of roleA must be an exact delta away from the tone of roleB.
    Exact,
    /// The tonal distance of roleA and roleB must be at most delta.
    Nearer,
    /// The tonal distance of roleA and roleB must be at least delta.
    Farther,
}

/// Describes the relationship in tones between a pair of DynamicColors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TonePolarity {
    /// The tone of roleA is always darker than the tone of roleB.
    Darker,
    /// The tone of roleA is always lighter than the tone of roleB.
    Lighter,
    /// The tone of roleA is darker than the tone of roleB in light mode, and
    /// lighter than the tone of roleB in dark mode.
    RelativeDarker,
    /// The tone of roleA is lighter than the tone of roleB in light mode, and
    /// darker than the tone of roleB in dark mode.
    RelativeLighter,
    /// Use `DeltaConstraint` instead; kept because the Java reference still
    /// constructs the 2021 spec's container/on-container pairs with it.
    #[deprecated = "use DeltaConstraint"]
    Nearer,
    /// Use `DeltaConstraint` instead; kept because the Java reference still
    /// defines it.
    #[deprecated = "use DeltaConstraint"]
    Farther,
}
