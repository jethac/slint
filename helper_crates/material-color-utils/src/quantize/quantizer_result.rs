// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::vec::Vec;

use crate::Argb;

/// Represents result of a quantizer run
///
/// `color_to_count` maps colors to their population in the image, in
/// first-insertion order.
#[derive(Debug)]
pub struct QuantizerResult {
    pub color_to_count: Vec<(Argb, i64)>,
}
