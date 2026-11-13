// Copyright © The Android Open Source Project, SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: Apache-2.0

//! Ports of the upstream `androidx.graphics.shapes` `commonTest` unit tests
//! (pinned commit 23327507f7fc7d5b19d65fec4b090f60c970079b).
//!
//! Mapping of upstream assertions: Kotlin's `assertThrows` becomes an `is_err()`
//! or `is_none()` check on the `Result`/`Option` the port returns; `assertEquals`
//! on floats becomes `assert_equalish` (same 1e-4 epsilon).

mod utils;

mod corner_rounding;
mod cubic;
mod feature_detector;
mod feature_mapping;
mod feature_serializer;
mod features;
mod float_mapping;
mod morph;
mod polygon;
mod polygon_measure;
mod polygon_validation;
mod rounded_polygon;
mod shapes;
mod svg_path_parser;
