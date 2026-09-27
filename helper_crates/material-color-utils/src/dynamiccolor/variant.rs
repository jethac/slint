// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Set of themes supported by Dynamic Color.
///
/// Instantiate the corresponding subclass, ex. `SchemeTonalSpot`, to create a
/// scheme with the given variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variant {
    /// A Dynamic Color theme that is intentionally detached from the source color.
    Monochrome,
    /// A Dynamic Color theme that is intentionally detached from the source color.
    Neutral,
    /// A Dynamic Color theme that maxes out colorfulness at each position in the
    /// Primary Tonal Palette.
    TonalSpot,
    /// A Dynamic Color theme that is intentionally detached from the source color.
    Vibrant,
    /// A Dynamic Color theme that is intentionally detached from the source color.
    Expressive,
    /// A Dynamic Color theme that is intentionally detached from the source color.
    /// Does not compensate for chroma or tone.
    Fidelity,
    /// A Dynamic Color theme that is intentionally detached from the source color.
    /// Does not compensate for chroma or tone.
    Content,
    /// A Dynamic Color theme that is intentionally detached from the source color.
    Rainbow,
    /// A Dynamic Color theme that is intentionally detached from the source color.
    FruitSalad,
    /// A Dynamic Color theme with 2 source colors, one primary, and one secondary.
    Cmf,
}
