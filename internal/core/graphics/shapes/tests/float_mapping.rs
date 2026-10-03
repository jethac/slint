// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Port of upstream `FloatMappingTest.kt`. `DoubleMapper(...)` throwing maps to
//! `DoubleMapper::new` returning `None`.

use super::super::mapping::DoubleMapper;
use super::utils::*;

fn validate_mapping(mapper: DoubleMapper, expected_function: impl Fn(f32) -> f32) {
    for i in 0..10000 {
        let source = i as f32 / 10000.;
        let target = expected_function(source);

        assert_equalish(target, mapper.map(source).unwrap());
        assert_equalish(source, mapper.map_back(target).unwrap());
    }
}

fn positive_modulo(x: f32, m: f32) -> f32 {
    let r = x % m;
    if r < 0. { r + m } else { r }
}

#[test]
fn identity_mapping_test() {
    // `DoubleMapper.Identity`: any two points on the (x, x) diagonal.
    validate_mapping(DoubleMapper::new(&[(0., 0.), (0.5, 0.5)]).unwrap(), |x| x);
}

#[test]
fn simple_mapping_test() {
    // Map the first half of the start source to the first quarter of the target.
    validate_mapping(DoubleMapper::new(&[(0., 0.), (0.5, 0.25)]).unwrap(), |x| {
        if x < 0.5 { x / 2. } else { (3. * x - 1.) / 2. }
    });
}

#[test]
fn target_wraps_test() {
    // mapping applies a "+ 0.5f"
    validate_mapping(DoubleMapper::new(&[(0., 0.5), (0.1, 0.6)]).unwrap(), |x| {
        positive_modulo(x + 0.5, 1.)
    });
}

#[test]
fn source_wraps_test() {
    // Values on the source wrap (this is still the "+ 0.5f" function)
    validate_mapping(DoubleMapper::new(&[(0.5, 0.), (0.1, 0.6)]).unwrap(), |x| {
        positive_modulo(x + 0.5, 1.)
    });
}

#[test]
fn both_wrap_test() {
    // Just the identity function
    validate_mapping(
        DoubleMapper::new(&[(0.5, 0.5), (0.75, 0.75), (0.1, 0.1), (0.49, 0.49)]).unwrap(),
        |x| x,
    );
}

#[test]
fn multiple_point_test() {
    validate_mapping(DoubleMapper::new(&[(0.4, 0.2), (0.5, 0.22), (0., 0.8)]).unwrap(), |x| {
        if x < 0.4 {
            positive_modulo(0.8 + x, 1.)
        } else if x < 0.5 {
            0.2 + (x - 0.4) / 5.
        } else {
            // maps a change of 0.5 in the source to a change 0.58 in the target, hence the 1.16
            0.22 + (x - 0.5) * 1.16
        }
    });
}

// The packaged graphics-shapes (1.0.1) `validateProgress` accepts a single
// wrap of the progress sequence — these inputs drop once and are therefore
// valid mappings.
#[test]
fn target_single_wrap_allowed() {
    assert!(DoubleMapper::new(&[(0., 0.), (0.3, 0.6), (0.6, 0.3), (0.9, 0.9)]).is_some());
}

#[test]
fn source_single_wrap_allowed() {
    assert!(DoubleMapper::new(&[(0., 0.), (0.6, 0.3), (0.3, 0.6), (0.9, 0.9)]).is_some());
}

#[test]
fn source_multiple_wraps_throw() {
    // Two drops exceed the single wrap `validateProgress` permits.
    assert!(
        DoubleMapper::new(&[(0., 0.), (0.6, 0.3), (0.5, 0.6), (0.9, 0.9), (0.8, 0.95)]).is_none()
    );
}
