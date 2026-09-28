// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BuiltinFunction, Callable, EasingCurve, EasingCurveCtor, Expression};
use crate::object_tree::{
    Component, ElementRc, PropertyAnimation, recurse_elem, visit_all_expressions,
};

/// Check the validity of expressions
///
/// - Check that the GetWindowScaleFactor and GetWindowDefaultFontSize are not called in a global
/// - Warn when an animation sets fields a physical spring ignores (`duration`,
///   `iteration-count`, `direction`)
pub fn check_expressions(doc: &crate::object_tree::Document, diag: &mut BuildDiagnostics) {
    for component in &doc.inner_components {
        visit_all_expressions(component, |e, _| check_expression(component, e, diag));
        recurse_elem(&component.root_element, &(), &mut |elem, _| {
            check_animation_elements(elem, diag)
        });
    }
}

fn check_animation_elements(elem: &ElementRc, diag: &mut BuildDiagnostics) {
    let elem = elem.borrow();
    let mut check = |anim: &ElementRc| {
        let anim = anim.borrow();
        let Some(easing) = anim.bindings.binding_cell_including_synthetic("easing") else {
            return;
        };
        let easing = easing.borrow();
        let physical = match &easing.expression {
            Expression::EasingCurve(EasingCurve::PhysicalSpring(..))
            | Expression::EasingCurveCtor { variant: EasingCurveCtor::PhysicalSpring, .. } => true,
            _ => false,
        };
        if !physical {
            return;
        }
        for field in ["duration", "iteration-count", "direction"] {
            let Some(binding) = anim.bindings.binding_cell_including_synthetic(field) else {
                continue;
            };
            let binding = binding.borrow();
            if !binding.from_source {
                continue;
            }
            let Some(span) = binding.span.clone() else { continue };
            diag.push_warning_with_span(
                format!("'{field}' is ignored on a physical spring: springs run until they settle"),
                span,
            );
        }
    };
    for (_, expr) in elem.bindings_including_synthetic() {
        match &expr.borrow().animation {
            Some(PropertyAnimation::Static(a)) => check(a),
            Some(PropertyAnimation::Transition { animations, .. }) => {
                for t in animations {
                    check(&t.animation);
                }
            }
            None => {}
        }
    }
    for t in &elem.transitions {
        for (_, _, a) in &t.property_animations {
            check(a);
        }
    }
}

fn check_expression(component: &Rc<Component>, e: &Expression, diag: &mut BuildDiagnostics) {
    if let Expression::FunctionCall { function: Callable::Builtin(b), source_location, .. } = e {
        match b {
            BuiltinFunction::GetWindowScaleFactor if component.is_global() => {
                diag.push_error("Cannot convert between logical and physical length in a global component, because the scale factor is not known".into(), source_location);
            }
            BuiltinFunction::GetWindowDefaultFontSize if component.is_global() => {
                diag.push_error("Cannot convert between rem and logical length in a global component, because the default font size is not known".into(), source_location);
            }
            _ => {}
        }
    }
    e.visit(|e| check_expression(component, e, diag))
}
