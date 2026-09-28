# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from slint import slint as native
from slint.slint import EasingCurve


def test_easing_curve_round_trip() -> None:
    compiler = native.Compiler()
    compdef = compiler.build_from_source(
        """
        export component Test {
            in-out property <easing> e1: linear;
            in-out property <easing> e2: cubic-bezier(0.1, 0.2, 0.3, 0.4);
            in-out property <easing> e3: spring(0.6, 800, 2);
            in-out property <easing> e4: spring(0.4);
            in-out property <easing> e5: ease-out-bounce;
        }
        """
    )
    instance = compdef.create()
    assert instance is not None

    assert instance.get_property("e1") == EasingCurve.linear()
    assert instance.get_property("e2") == EasingCurve.cubic_bezier(0.1, 0.2, 0.3, 0.4)
    assert instance.get_property("e3") == EasingCurve.spring(0.6, 800, 2)
    assert instance.get_property("e4") == EasingCurve.spring_bounce(0.4)
    assert instance.get_property("e5") == EasingCurve.from_name("ease-out-bounce")

    # Each Python value comes back unchanged.
    for prop, curve in [
        ("e1", EasingCurve.spring(0.9, 700)),
        ("e2", EasingCurve.linear()),
        ("e3", EasingCurve.spring_bounce(0.5)),
        ("e4", EasingCurve.cubic_bezier(0.0, 0.0, 1.0, 1.0)),
        ("e5", EasingCurve.from_name("ease-in-elastic")),
    ]:
        instance.set_property(prop, curve)
        assert instance.get_property(prop) == curve
