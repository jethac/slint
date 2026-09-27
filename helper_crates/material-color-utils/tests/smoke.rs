// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use material_color_utils::dynamiccolor::{Platform, SpecVersion};
use material_color_utils::hct::Hct;
use material_color_utils::scheme::SchemeTonalSpot;

#[test]
fn smoke() {
    for (spec, (primary, surface)) in [
        (SpecVersion::Spec2021, (0xff445e91i64, 0xfff9f9ffi64)),
        (SpecVersion::Spec2025, (0xff495f8b, 0xfffaf9fe)),
        (SpecVersion::Spec2026, (0xff495f8b, 0xfffaf9fe)),
    ] {
        let s =
            SchemeTonalSpot::from_hct(Hct::from_int(0xff4285f4), false, 0.0, spec, Platform::Phone);
        assert_eq!(s.primary(), primary, "{spec:?} primary");
        assert_eq!(s.surface(), surface, "{spec:?} surface");
    }
}
