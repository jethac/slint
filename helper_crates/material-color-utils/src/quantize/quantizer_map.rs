// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::Argb;

use super::quantizer_result::QuantizerResult;

/// An insertion-ordered ARGB→count map.
///
/// Iteration order is load-bearing: QuantizerWsmeans consumes the entries in
/// the exact order Java's `LinkedHashMap` yields them (first-insertion
/// order), and any deviation changes the cluster assignments.
#[derive(Debug, Default)]
pub(crate) struct InsertionMap {
    entries: Vec<(Argb, i64)>,
    index: BTreeMap<Argb, usize>,
}

impl InsertionMap {
    /// `pixelByCount.merge(pixel, 1, Integer::sum)` — insert `1` or add to the
    /// existing count.
    pub fn increment(&mut self, key: Argb) {
        if let Some(&idx) = self.index.get(&key) {
            self.entries[idx].1 += 1;
        } else {
            self.index.insert(key, self.entries.len());
            self.entries.push((key, 1));
        }
    }

    pub fn get(&self, key: Argb) -> Option<i64> {
        self.index.get(&key).map(|&idx| self.entries[idx].1)
    }

    /// `map[key] = count` — insert or overwrite, preserving insertion order.
    pub fn set(&mut self, key: Argb, count: i64) {
        if let Some(&idx) = self.index.get(&key) {
            self.entries[idx].1 = count;
        } else {
            self.index.insert(key, self.entries.len());
            self.entries.push((key, count));
        }
    }

    /// `mapTo(pixelByCount) { pixel to count }` — entries in insertion order.
    pub fn entries(&self) -> &[(Argb, i64)] {
        &self.entries
    }
}

/// Creates a map with keys of all colors provided in a given image, and a value
/// of the number of times that color appears in the image.
pub struct QuantizerMap;

impl QuantizerMap {
    /// `pixels`: array of ARGB representations of pixels in the image.
    /// `max_colors`: unused, kept for parity with the `Quantizer` interface.
    ///
    /// Returns: an ordered list of (color, count) pairs, in first-insertion
    /// order — matching Java `Map<Argb, Int>` built on `LinkedHashMap`.
    pub fn quantize(pixels: &[Argb], _max_colors: i32) -> QuantizerResult {
        let mut pixel_by_count = InsertionMap::default();
        for &pixel in pixels {
            pixel_by_count.increment(pixel);
        }
        QuantizerResult { color_to_count: pixel_by_count.entries().to_vec() }
    }
}
