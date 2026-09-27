// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::vec::Vec;

use crate::Argb;
use crate::math;
use crate::utils::ColorUtils;

use super::quantizer_map::QuantizerMap;
use super::quantizer_result::QuantizerResult;

/// An image quantizer that divides the image's pixels into clusters by
/// recursively cutting an RGB cube, based on the weight of pixels in each area
/// of the cube.
///
/// The algorithm was described by Xiaolin Wu in Graphic Gems II, published in
/// 1991.
pub struct QuantizerWu {
    weights: Vec<i32>,
    moments_r: Vec<i32>,
    moments_g: Vec<i32>,
    moments_b: Vec<i32>,
    moments: Vec<f64>,
    cubes: Vec<Box>,
}

const INDEX_BITS: i32 = 5;
const INDEX_COUNT: i32 = 33; // ((1 << INDEX_BITS) + 1)
const TOTAL_SIZE: usize = 35937; // INDEX_COUNT * INDEX_COUNT * INDEX_COUNT

#[derive(Clone, Copy, Default)]
pub(crate) struct Box {
    r0: i32,
    r1: i32,
    g0: i32,
    g1: i32,
    b0: i32,
    b1: i32,
    vol: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Red,
    Green,
    Blue,
}

#[derive(Clone, Copy)]
struct MaximizeResult {
    cut_location: i32, // < 0 if cut impossible
    maximum: f64,
}

fn get_index(r: i32, g: i32, b: i32) -> usize {
    ((r << (INDEX_BITS * 2)) + (r << (INDEX_BITS + 1)) + r + (g << INDEX_BITS) + g + b) as usize
}

fn volume(moment: &[i32], cube: &Box) -> i32 {
    moment[get_index(cube.r1, cube.g1, cube.b1)]
        .wrapping_sub(moment[get_index(cube.r1, cube.g1, cube.b0)])
        .wrapping_sub(moment[get_index(cube.r1, cube.g0, cube.b1)])
        .wrapping_add(moment[get_index(cube.r1, cube.g0, cube.b0)])
        .wrapping_sub(moment[get_index(cube.r0, cube.g1, cube.b1)])
        .wrapping_add(moment[get_index(cube.r0, cube.g1, cube.b0)])
        .wrapping_add(moment[get_index(cube.r0, cube.g0, cube.b1)])
        .wrapping_sub(moment[get_index(cube.r0, cube.g0, cube.b0)])
}

fn bottom(cube: &Box, direction: Direction, moment: &[i32]) -> i32 {
    match direction {
        Direction::Red => {
            -moment[get_index(cube.r0, cube.g1, cube.b1)]
                + moment[get_index(cube.r0, cube.g1, cube.b0)]
                + moment[get_index(cube.r0, cube.g0, cube.b1)]
                - moment[get_index(cube.r0, cube.g0, cube.b0)]
        }
        Direction::Green => {
            -moment[get_index(cube.r1, cube.g0, cube.b1)]
                + moment[get_index(cube.r1, cube.g0, cube.b0)]
                + moment[get_index(cube.r0, cube.g0, cube.b1)]
                - moment[get_index(cube.r0, cube.g0, cube.b0)]
        }
        Direction::Blue => {
            -moment[get_index(cube.r1, cube.g1, cube.b0)]
                + moment[get_index(cube.r1, cube.g0, cube.b0)]
                + moment[get_index(cube.r0, cube.g1, cube.b0)]
                - moment[get_index(cube.r0, cube.g0, cube.b0)]
        }
    }
}

fn top(cube: &Box, direction: Direction, position: i32, moment: &[i32]) -> i32 {
    match direction {
        Direction::Red => {
            moment[get_index(position, cube.g1, cube.b1)]
                - moment[get_index(position, cube.g1, cube.b0)]
                - moment[get_index(position, cube.g0, cube.b1)]
                + moment[get_index(position, cube.g0, cube.b0)]
        }
        Direction::Green => {
            moment[get_index(cube.r1, position, cube.b1)]
                - moment[get_index(cube.r1, position, cube.b0)]
                - moment[get_index(cube.r0, position, cube.b1)]
                + moment[get_index(cube.r0, position, cube.b0)]
        }
        Direction::Blue => {
            moment[get_index(cube.r1, cube.g1, position)]
                - moment[get_index(cube.r1, cube.g0, position)]
                - moment[get_index(cube.r0, cube.g1, position)]
                + moment[get_index(cube.r0, cube.g0, position)]
        }
    }
}

impl QuantizerWu {
    pub fn new() -> Self {
        Self {
            weights: Vec::new(),
            moments_r: Vec::new(),
            moments_g: Vec::new(),
            moments_b: Vec::new(),
            moments: Vec::new(),
            cubes: Vec::new(),
        }
    }

    /// `pixels`: Pixels in the image as ARGB ints.
    /// `max_colors`: The maximum number of colors to return.
    ///
    /// Returns: Map of colors (as ARGB ints) to the number of times the color
    /// appears in the image.
    pub fn quantize(&mut self, pixels: &[Argb], max_colors: i32) -> QuantizerResult {
        let map_result = QuantizerMap::quantize(pixels, max_colors);
        self.construct_histogram(&map_result.color_to_count);
        self.create_moments();
        let create_boxes_result = self.create_boxes(max_colors);
        let colors = self.create_result(create_boxes_result);
        let color_to_count = colors.into_iter().map(|color| (color, 0)).collect();
        QuantizerResult { color_to_count }
    }

    fn construct_histogram(&mut self, pixels: &[(Argb, i64)]) {
        self.weights = alloc::vec![0; TOTAL_SIZE];
        self.moments_r = alloc::vec![0; TOTAL_SIZE];
        self.moments_g = alloc::vec![0; TOTAL_SIZE];
        self.moments_b = alloc::vec![0; TOTAL_SIZE];
        self.moments = alloc::vec![0.0; TOTAL_SIZE];
        for &(pixel, count) in pixels {
            let red = ColorUtils::red_from_argb(pixel) as i32;
            let green = ColorUtils::green_from_argb(pixel) as i32;
            let blue = ColorUtils::blue_from_argb(pixel) as i32;
            let bits_to_remove = 8 - INDEX_BITS;
            let i_r = (red >> bits_to_remove) + 1;
            let i_g = (green >> bits_to_remove) + 1;
            let i_b = (blue >> bits_to_remove) + 1;
            let index = get_index(i_r, i_g, i_b);
            let count = count as i32;
            self.weights[index] = self.weights[index].wrapping_add(count);
            self.moments_r[index] = self.moments_r[index].wrapping_add(red.wrapping_mul(count));
            self.moments_g[index] = self.moments_g[index].wrapping_add(green.wrapping_mul(count));
            self.moments_b[index] = self.moments_b[index].wrapping_add(blue.wrapping_mul(count));
            let squared = red
                .wrapping_mul(red)
                .wrapping_add(green.wrapping_mul(green))
                .wrapping_add(blue.wrapping_mul(blue));
            self.moments[index] += count.wrapping_mul(squared) as f64;
        }
    }

    fn create_moments(&mut self) {
        for r in 1..INDEX_COUNT {
            let mut area = [0i32; INDEX_COUNT as usize];
            let mut area_r = [0i32; INDEX_COUNT as usize];
            let mut area_g = [0i32; INDEX_COUNT as usize];
            let mut area_b = [0i32; INDEX_COUNT as usize];
            let mut area2 = [0.0f64; INDEX_COUNT as usize];
            for g in 1..INDEX_COUNT {
                let mut line = 0i32;
                let mut line_r = 0i32;
                let mut line_g = 0i32;
                let mut line_b = 0i32;
                let mut line2 = 0.0f64;
                for b in 1..INDEX_COUNT {
                    let index = get_index(r, g, b);
                    line = line.wrapping_add(self.weights[index]);
                    line_r = line_r.wrapping_add(self.moments_r[index]);
                    line_g = line_g.wrapping_add(self.moments_g[index]);
                    line_b = line_b.wrapping_add(self.moments_b[index]);
                    line2 += self.moments[index];
                    area[b as usize] = area[b as usize].wrapping_add(line);
                    area_r[b as usize] = area_r[b as usize].wrapping_add(line_r);
                    area_g[b as usize] = area_g[b as usize].wrapping_add(line_g);
                    area_b[b as usize] = area_b[b as usize].wrapping_add(line_b);
                    area2[b as usize] += line2;
                    let previous_index = get_index(r - 1, g, b);
                    self.weights[index] =
                        self.weights[previous_index].wrapping_add(area[b as usize]);
                    self.moments_r[index] =
                        self.moments_r[previous_index].wrapping_add(area_r[b as usize]);
                    self.moments_g[index] =
                        self.moments_g[previous_index].wrapping_add(area_g[b as usize]);
                    self.moments_b[index] =
                        self.moments_b[previous_index].wrapping_add(area_b[b as usize]);
                    self.moments[index] = self.moments[previous_index] + area2[b as usize];
                }
            }
        }
    }

    fn create_boxes(&mut self, max_color_count: i32) -> usize {
        self.cubes = alloc::vec![Box::default(); max_color_count as usize];
        let mut volume_variance = alloc::vec![0.0f64; max_color_count as usize];
        let first_box = &mut self.cubes[0];
        first_box.r1 = INDEX_COUNT - 1;
        first_box.g1 = INDEX_COUNT - 1;
        first_box.b1 = INDEX_COUNT - 1;
        let mut generated_color_count = max_color_count;
        let mut next = 0usize;
        let mut i = 1i32;
        while i < max_color_count {
            if self.cut(next, i as usize) {
                volume_variance[next] =
                    if self.cubes[next].vol > 1 { self.variance(&self.cubes[next]) } else { 0.0 };
                volume_variance[i as usize] = if self.cubes[i as usize].vol > 1 {
                    self.variance(&self.cubes[i as usize])
                } else {
                    0.0
                };
            } else {
                volume_variance[next] = 0.0;
                i -= 1;
            }
            next = 0;
            let mut temp = volume_variance[0];
            for j in 1..=i {
                if volume_variance[j as usize] > temp {
                    temp = volume_variance[j as usize];
                    next = j as usize;
                }
            }
            if temp <= 0.0 {
                generated_color_count = i + 1;
                break;
            }
            i += 1;
        }
        generated_color_count as usize
    }

    fn create_result(&mut self, color_count: usize) -> Vec<Argb> {
        let mut colors: Vec<Argb> = Vec::new();
        for i in 0..color_count {
            let cube = self.cubes[i];
            let weight = volume(&self.weights, &cube);
            if weight > 0 {
                let r = math::round_to_int(volume(&self.moments_r, &cube) as f64 / weight as f64)
                    as i32;
                let g = math::round_to_int(volume(&self.moments_g, &cube) as f64 / weight as f64)
                    as i32;
                let b = math::round_to_int(volume(&self.moments_b, &cube) as f64 / weight as f64)
                    as i32;
                let color: Argb = ((255 << 24) as i64)
                    | (((r & 0x0ff) << 16) as i64)
                    | (((g & 0x0ff) << 8) as i64)
                    | ((b & 0x0ff) as i64);
                colors.push(color);
            }
        }
        colors
    }

    fn variance(&self, cube: &Box) -> f64 {
        let dr = volume(&self.moments_r, cube);
        let dg = volume(&self.moments_g, cube);
        let db = volume(&self.moments_b, cube);
        let xx = self.moments[get_index(cube.r1, cube.g1, cube.b1)]
            - self.moments[get_index(cube.r1, cube.g1, cube.b0)]
            - self.moments[get_index(cube.r1, cube.g0, cube.b1)]
            + self.moments[get_index(cube.r1, cube.g0, cube.b0)]
            - self.moments[get_index(cube.r0, cube.g1, cube.b1)]
            + self.moments[get_index(cube.r0, cube.g1, cube.b0)]
            + self.moments[get_index(cube.r0, cube.g0, cube.b1)]
            - self.moments[get_index(cube.r0, cube.g0, cube.b0)];
        let hypotenuse =
            dr.wrapping_mul(dr).wrapping_add(dg.wrapping_mul(dg)).wrapping_add(db.wrapping_mul(db));
        let volume = volume(&self.weights, cube);
        xx - hypotenuse as f64 / volume as f64
    }

    /// Kotlin `cut(one: Box, two: Box)` mutates both boxes; here the boxes are
    /// indexed into `self.cubes`.
    fn cut(&mut self, one_i: usize, two_i: usize) -> bool {
        let one = self.cubes[one_i];
        let whole_r = volume(&self.moments_r, &one);
        let whole_g = volume(&self.moments_g, &one);
        let whole_b = volume(&self.moments_b, &one);
        let whole_w = volume(&self.weights, &one);
        let max_r_result = self.maximize(
            &one,
            Direction::Red,
            one.r0 + 1,
            one.r1,
            whole_r,
            whole_g,
            whole_b,
            whole_w,
        );
        let max_g_result = self.maximize(
            &one,
            Direction::Green,
            one.g0 + 1,
            one.g1,
            whole_r,
            whole_g,
            whole_b,
            whole_w,
        );
        let max_b_result = self.maximize(
            &one,
            Direction::Blue,
            one.b0 + 1,
            one.b1,
            whole_r,
            whole_g,
            whole_b,
            whole_w,
        );
        let max_r = max_r_result.maximum;
        let max_g = max_g_result.maximum;
        let max_b = max_b_result.maximum;
        let cut_direction = if max_r >= max_g && max_r >= max_b {
            if max_r_result.cut_location < 0 {
                return false;
            }
            Direction::Red
        } else if max_g >= max_r && max_g >= max_b {
            Direction::Green
        } else {
            Direction::Blue
        };
        self.cubes[two_i].r1 = one.r1;
        self.cubes[two_i].g1 = one.g1;
        self.cubes[two_i].b1 = one.b1;
        match cut_direction {
            Direction::Red => {
                self.cubes[one_i].r1 = max_r_result.cut_location;
                self.cubes[two_i].r0 = self.cubes[one_i].r1;
                self.cubes[two_i].g0 = self.cubes[one_i].g0;
                self.cubes[two_i].b0 = self.cubes[one_i].b0;
            }
            Direction::Green => {
                self.cubes[one_i].g1 = max_g_result.cut_location;
                self.cubes[two_i].r0 = self.cubes[one_i].r0;
                self.cubes[two_i].g0 = self.cubes[one_i].g1;
                self.cubes[two_i].b0 = self.cubes[one_i].b0;
            }
            Direction::Blue => {
                self.cubes[one_i].b1 = max_b_result.cut_location;
                self.cubes[two_i].r0 = self.cubes[one_i].r0;
                self.cubes[two_i].g0 = self.cubes[one_i].g0;
                self.cubes[two_i].b0 = self.cubes[one_i].b1;
            }
        }
        let one = self.cubes[one_i];
        self.cubes[one_i].vol = (one.r1 - one.r0) * (one.g1 - one.g0) * (one.b1 - one.b0);
        let two = self.cubes[two_i];
        self.cubes[two_i].vol = (two.r1 - two.r0) * (two.g1 - two.g0) * (two.b1 - two.b0);
        true
    }

    fn maximize(
        &mut self,
        cube: &Box,
        direction: Direction,
        first: i32,
        last: i32,
        whole_r: i32,
        whole_g: i32,
        whole_b: i32,
        whole_w: i32,
    ) -> MaximizeResult {
        let bottom_r = bottom(cube, direction, &self.moments_r);
        let bottom_g = bottom(cube, direction, &self.moments_g);
        let bottom_b = bottom(cube, direction, &self.moments_b);
        let bottom_w = bottom(cube, direction, &self.weights);
        let mut max = 0.0f64;
        let mut cut = -1i32;
        for i in first..last {
            let mut half_r = bottom_r.wrapping_add(top(cube, direction, i, &self.moments_r));
            let mut half_g = bottom_g.wrapping_add(top(cube, direction, i, &self.moments_g));
            let mut half_b = bottom_b.wrapping_add(top(cube, direction, i, &self.moments_b));
            let mut half_w = bottom_w.wrapping_add(top(cube, direction, i, &self.weights));
            if half_w == 0 {
                continue;
            }
            let mut temp_numerator = half_r
                .wrapping_mul(half_r)
                .wrapping_add(half_g.wrapping_mul(half_g))
                .wrapping_add(half_b.wrapping_mul(half_b))
                as f64;
            let mut temp_denominator = half_w as f64;
            let mut temp = temp_numerator / temp_denominator;
            half_r = whole_r.wrapping_sub(half_r);
            half_g = whole_g.wrapping_sub(half_g);
            half_b = whole_b.wrapping_sub(half_b);
            half_w = whole_w.wrapping_sub(half_w);
            if half_w == 0 {
                continue;
            }
            temp_numerator = half_r
                .wrapping_mul(half_r)
                .wrapping_add(half_g.wrapping_mul(half_g))
                .wrapping_add(half_b.wrapping_mul(half_b)) as f64;
            temp_denominator = half_w as f64;
            temp += temp_numerator / temp_denominator;
            if temp > max {
                max = temp;
                cut = i;
            }
        }
        MaximizeResult { cut_location: cut, maximum: max }
    }
}
