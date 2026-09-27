// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::vec::Vec;
use core::cmp::min;

use crate::Argb;
use crate::math::sqrt;

use super::java_random::JavaRandom;
use super::point_provider::PointProvider;
use super::point_provider_lab::PointProviderLab;
use super::quantizer_map::InsertionMap;

/// An image quantizer that improves on the speed of a standard K-Means
/// algorithm by implementing several optimizations, including deduping
/// identical pixels and a triangle inequality rule that reduces the number of
/// comparisons needed to identify which cluster a point should be moved to.
///
/// Wsmeans stands for Weighted Square Means.
///
/// This algorithm was designed by M. Emre Celebi, and was found in their 2011
/// paper, Improving the Performance of K-Means for Color Quantization.
/// https://arxiv.org/abs/1101.0395
pub struct QuantizerWsmeans;

const MAX_ITERATIONS: usize = 10;
const MIN_MOVEMENT_DISTANCE: f64 = 3.0;

#[derive(Clone, Copy)]
struct Distance {
    index: i32,
    distance: f64,
}

impl Default for Distance {
    fn default() -> Self {
        Self { index: -1, distance: -1.0 }
    }
}

impl QuantizerWsmeans {
    /// Reduce the number of colors needed to represented the input, minimizing
    /// the difference between the original image and the recolored image.
    ///
    /// `input_pixels`: Colors in ARGB format.
    /// `starting_clusters`: Defines the initial state of the quantizer. Passing
    /// an empty array is fine, the implementation will create its own initial
    /// state that leads to reproducible results for the same inputs. Passing an
    /// array that is the result of Wu quantization leads to higher quality
    /// results.
    /// `max_colors`: The number of colors to divide the image into. A lower
    /// number of colors may be returned.
    ///
    /// Returns: ordered list of (color in ARGB, pixel count) pairs — matching
    /// Kotlin's insertion-ordered `Map<Argb, Int>`.
    pub fn quantize(
        input_pixels: &[Argb],
        starting_clusters: &[Argb],
        max_colors: i32,
    ) -> Vec<(Argb, i64)> {
        // Uses a seeded random number generator to ensure consistent results.
        let mut random = JavaRandom::new(0x42688);
        let mut pixel_to_count = InsertionMap::default();
        let mut points: Vec<[f64; 4]> = Vec::with_capacity(input_pixels.len());
        let mut pixels: Vec<Argb> = Vec::with_capacity(input_pixels.len());
        let point_provider = PointProviderLab;
        for &input_pixel in input_pixels {
            if pixel_to_count.get(input_pixel).is_none() {
                points.push(point_provider.from_int(input_pixel));
                pixels.push(input_pixel);
                pixel_to_count.increment(input_pixel);
            } else {
                pixel_to_count.increment(input_pixel);
            }
        }
        let point_count = pixels.len();
        let mut counts = alloc::vec![0i64; point_count];
        for i in 0..point_count {
            let pixel = pixels[i];
            let count = pixel_to_count.get(pixel).unwrap();
            counts[i] = count;
        }
        let mut cluster_count = min(max_colors, point_count as i32);
        if !starting_clusters.is_empty() {
            cluster_count = min(cluster_count, starting_clusters.len() as i32);
        }
        let mut clusters: Vec<[f64; 4]> = alloc::vec![[0.0; 4]; cluster_count.max(0) as usize];
        let mut clusters_created = 0;
        for i in 0..starting_clusters.len() {
            clusters[i] = point_provider.from_int(starting_clusters[i]);
            clusters_created += 1;
        }
        let additional_clusters_needed = cluster_count as usize - clusters_created;
        if starting_clusters.is_empty() && additional_clusters_needed > 0 {
            // The TypeScript implementation fills the remaining clusters with
            // random points (the Kotlin implementation iterates without a
            // body — a no-op). The seeded RNG keeps this deterministic.
            for i in 0..additional_clusters_needed {
                let l = random.next_double() * 100.0;
                let a = random.next_double() * (100.0 - (-100.0) + 1.0) + -100.0;
                let b = random.next_double() * (100.0 - (-100.0) + 1.0) + -100.0;
                clusters[clusters_created + i] = [l, a, b, 0.0];
            }
        }
        let cluster_count = cluster_count as usize;
        let mut cluster_indices: Vec<i32> = Vec::with_capacity(point_count);
        for _ in 0..point_count {
            cluster_indices
                .push(crate::math::floor(random.next_double() * cluster_count as f64) as i32);
        }
        // The TypeScript implementation sorts distanceToIndexMatrix rows with
        // `Array.sort()` and no comparator, which stringifies the Distance
        // objects to "[object Object]" for every element — a no-op — so the
        // matrix keeps insertion order (and indexMatrix is never read).
        let mut distance_to_index_matrix: Vec<Vec<Distance>> =
            alloc::vec![alloc::vec![Distance::default(); cluster_count]; cluster_count];
        let mut pixel_count_sums = alloc::vec![0i64; cluster_count];
        for iteration in 0..MAX_ITERATIONS {
            for i in 0..cluster_count {
                for j in (i + 1)..cluster_count {
                    let distance = point_provider.distance(clusters[i], clusters[j]);
                    distance_to_index_matrix[j][i].distance = distance;
                    distance_to_index_matrix[j][i].index = i as i32;
                    distance_to_index_matrix[i][j].distance = distance;
                    distance_to_index_matrix[i][j].index = j as i32;
                }
            }
            let mut points_moved = 0;
            for i in 0..point_count {
                let point = points[i];
                let previous_cluster_index = cluster_indices[i] as usize;
                let previous_cluster = clusters[previous_cluster_index];
                let previous_distance = point_provider.distance(point, previous_cluster);
                let mut minimum_distance = previous_distance;
                let mut new_cluster_index = -1i32;
                for j in 0..cluster_count {
                    if distance_to_index_matrix[previous_cluster_index][j].distance
                        >= 4.0 * previous_distance
                    {
                        continue;
                    }
                    let distance = point_provider.distance(point, clusters[j]);
                    if distance < minimum_distance {
                        minimum_distance = distance;
                        new_cluster_index = j as i32;
                    }
                }
                if new_cluster_index != -1 {
                    let distance_change = (sqrt(minimum_distance) - sqrt(previous_distance)).abs();
                    if distance_change > MIN_MOVEMENT_DISTANCE {
                        points_moved += 1;
                        cluster_indices[i] = new_cluster_index;
                    }
                }
            }
            if points_moved == 0 && iteration != 0 {
                break;
            }
            let mut component_a_sums = alloc::vec![0.0f64; cluster_count];
            let mut component_b_sums = alloc::vec![0.0f64; cluster_count];
            let mut component_c_sums = alloc::vec![0.0f64; cluster_count];
            pixel_count_sums.fill(0);
            for i in 0..point_count {
                let cluster_index = cluster_indices[i] as usize;
                let point = points[i];
                let count = counts[i];
                pixel_count_sums[cluster_index] += count;
                component_a_sums[cluster_index] += point[0] * count as f64;
                component_b_sums[cluster_index] += point[1] * count as f64;
                component_c_sums[cluster_index] += point[2] * count as f64;
            }
            for i in 0..cluster_count {
                let count = pixel_count_sums[i];
                if count == 0 {
                    clusters[i] = [0.0, 0.0, 0.0, 0.0];
                    continue;
                }
                let a = component_a_sums[i] / count as f64;
                let b = component_b_sums[i] / count as f64;
                let c = component_c_sums[i] / count as f64;
                clusters[i] = [a, b, c, 0.0];
            }
        }
        let mut argb_to_population = InsertionMap::default();
        for i in 0..cluster_count {
            let count = pixel_count_sums[i];
            if count == 0 {
                continue;
            }
            let possible_new_cluster = point_provider.to_int(clusters[i]);
            if argb_to_population.get(possible_new_cluster).is_some() {
                continue;
            }
            argb_to_population.set(possible_new_cluster, count);
        }
        argb_to_population.entries().to_vec()
    }
}
