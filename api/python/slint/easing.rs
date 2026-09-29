// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::prelude::*;

use i_slint_core::animations::EasingCurve;

/// An easing curve, as the `easing` type in `.slint`.
///
/// Construct instances with the class methods: `EasingCurve.linear()`,
/// `EasingCurve.cubic_bezier(x1, y1, x2, y2)`, `EasingCurve.spring(damping_ratio,
/// stiffness[, mass])`, `EasingCurve.spring_bounce(bounce)` and
/// `EasingCurve.from_name("ease-out-elastic")`.
#[pyclass(name = "EasingCurve", skip_from_py_object)]
#[derive(Clone)]
pub struct PyEasingCurve {
    pub curve: EasingCurve,
}

#[pymethods]
impl PyEasingCurve {
    /// A linear easing curve.
    #[staticmethod]
    fn linear() -> Self {
        Self { curve: EasingCurve::Linear }
    }

    /// A cubic bezier curve with the four control point coordinates `x1`, `y1`,
    /// `x2`, `y2`.
    #[staticmethod]
    fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { curve: EasingCurve::CubicBezier([x1, y1, x2, y2]) }
    }

    /// A physical spring — `spring(damping_ratio, stiffness[, mass])` in `.slint`:
    /// `damping_ratio` below `1` bounces, `1` is critically damped, above `1`
    /// overshoots nothing. The spring runs until it settles; no `duration`
    /// applies. `mass` defaults to `1`.
    #[staticmethod]
    #[pyo3(signature = (damping_ratio, stiffness, mass = None))]
    fn spring(damping_ratio: f32, stiffness: f32, mass: Option<f32>) -> PyResult<Self> {
        if damping_ratio < 0.0 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "spring damping_ratio must be non-negative",
            ));
        }
        if stiffness <= 0.0 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "spring stiffness must be positive",
            ));
        }
        let mass = mass.unwrap_or(1.0);
        if mass <= 0.0 {
            return Err(pyo3::exceptions::PyValueError::new_err("spring mass must be positive"));
        }
        Ok(Self { curve: EasingCurve::PhysicalSpring { damping_ratio, stiffness, mass } })
    }

    /// The legacy `spring(bounce)` easing, where `bounce` between `-1` and `1`
    /// adds springiness to a `duration`-based animation.
    #[staticmethod]
    fn spring_bounce(bounce: f32) -> PyResult<Self> {
        if !(-1.0..=1.0).contains(&bounce) {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "spring_bounce's bounce must be between -1 and 1",
            ));
        }
        Ok(Self { curve: EasingCurve::Spring(bounce) })
    }

    /// A named curve: `linear` or one of `ease-{in,out,in-out}-{elastic,bounce}`.
    #[staticmethod]
    fn from_name(name: &str) -> PyResult<Self> {
        Ok(Self {
            curve: match name {
                "linear" => EasingCurve::Linear,
                "ease-in-elastic" => EasingCurve::EaseInElastic,
                "ease-out-elastic" => EasingCurve::EaseOutElastic,
                "ease-in-out-elastic" => EasingCurve::EaseInOutElastic,
                "ease-in-bounce" => EasingCurve::EaseInBounce,
                "ease-out-bounce" => EasingCurve::EaseOutBounce,
                "ease-in-out-bounce" => EasingCurve::EaseInOutBounce,
                _ => {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "'{name}' is not a named easing curve"
                    )));
                }
            },
        })
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.curve == other.curve
    }

    fn __repr__(&self) -> String {
        format!("slint.EasingCurve({:?})", self.curve)
    }
}
