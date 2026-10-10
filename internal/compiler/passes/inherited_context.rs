// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Lower ancestor-property reads to ordinary reactive component inputs.

use crate::diagnostics::{BuildDiagnostics, SourceLocation};
use crate::expression_tree::{ConditionLocation, Expression, NamedReference};
use crate::langtype::{ElementType, PropertyLookupMode, Type};
use crate::lookup::LookupCtx;
use crate::object_tree::{Document, ElementRc, PropertyDeclaration, PropertyVisibility};
use crate::parser::NodeOrToken;
use crate::symbol_counters::SymbolCounters;
use smol_str::{SmolStr, format_smolstr};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug)]
pub struct InheritanceRequest {
    owner: std::rc::Weak<std::cell::RefCell<crate::object_tree::Element>>,
    name: SmolStr,
    context: SmolStr,
    fallback: Expression,
    location: SourceLocation,
}

pub fn lower_inherit(
    arguments: Vec<(Expression, Option<NodeOrToken>)>,
    location: &SourceLocation,
    ctx: &mut LookupCtx<'_>,
) -> Expression {
    let mut arguments = arguments.into_iter();
    let (Some((Expression::StringLiteral(context), _)), Some((fallback, _)), None) =
        (arguments.next(), arguments.next(), arguments.next())
    else {
        ctx.diag.push_error(
            "inherit expects a literal property name and a fallback value".into(),
            location,
        );
        return Expression::Invalid;
    };
    if context.is_empty() {
        ctx.diag.push_error("The inherited property name cannot be empty".into(), location);
        return Expression::Invalid;
    }
    let Some(component) = ctx
        .component_scope
        .last()
        .and_then(|element| element.borrow().enclosing_component.upgrade())
        .filter(|component| !component.is_global())
    else {
        ctx.diag.push_error("inherit requires an element scope".into(), location);
        return Expression::Invalid;
    };
    let name = ctx.symbol_counters.generate_name("inherited-context-");
    let ty = fallback.ty();
    component.root_element.borrow().inherited_requests.borrow_mut().push(InheritanceRequest {
        owner: Rc::downgrade(ctx.component_scope.last().unwrap()),
        name: name.clone(),
        context: context.replace('_', "-").into(),
        fallback,
        location: location.clone(),
    });
    Expression::ReadLocalVariable { name, ty }
}

fn presence(name: &str) -> SmolStr {
    format_smolstr!("{name}-provided")
}

fn declare_request(root: &ElementRc, name: SmolStr, context: SmolStr, ty: Type) {
    let mut element = root.borrow_mut();
    element.property_declarations.insert(
        presence(&name),
        PropertyDeclaration {
            property_type: Type::Bool,
            visibility: PropertyVisibility::Input,
            ..Default::default()
        },
    );
    element.property_declarations.insert(
        name,
        PropertyDeclaration {
            property_type: ty,
            inherited_context: Some(context),
            visibility: PropertyVisibility::Input,
            ..Default::default()
        },
    );
}

pub fn materialize_requests(doc: &Document) {
    for component in &doc.inner_components {
        let requests =
            std::mem::take(&mut *component.root_element.borrow().inherited_requests.borrow_mut());
        let mut replacements = HashMap::new();
        for request in requests {
            let owner = request.owner.upgrade().expect("inheritance owner is in the document");
            declare_request(&owner, request.name.clone(), request.context, request.fallback.ty());
            replacements.insert(
                request.name.clone(),
                Expression::Condition {
                    condition: Box::new(Expression::PropertyReference(NamedReference::new(
                        &owner,
                        presence(&request.name),
                    ))),
                    true_expr: Box::new(Expression::PropertyReference(NamedReference::new(
                        &owner,
                        request.name,
                    ))),
                    false_expr: Box::new(request.fallback),
                    source_location: Some(ConditionLocation::Question(request.location)),
                },
            );
        }
        if replacements.is_empty() {
            continue;
        }
        fn replace(expression: &mut Expression, replacements: &HashMap<SmolStr, Expression>) {
            if let Expression::ReadLocalVariable { name, .. } = expression {
                if let Some(replacement) = replacements.get(name) {
                    *expression = replacement.clone();
                }
            }
            expression.visit_mut(|child| replace(child, replacements));
        }
        crate::object_tree::recurse_elem_no_borrow(
            &component.root_element,
            &(),
            &mut |element, _| {
                crate::object_tree::visit_element_expressions(element, |expression, _, _| {
                    replace(expression, &replacements)
                });
            },
        );
    }
}

fn requests(element: &ElementRc) -> Vec<(SmolStr, SmolStr, Type)> {
    let mut result: Vec<_> = element
        .borrow()
        .property_declarations
        .iter()
        .filter_map(|(name, declaration)| {
            declaration
                .inherited_context
                .as_ref()
                .map(|context| (name.clone(), context.clone(), declaration.property_type.clone()))
        })
        .collect();
    if let ElementType::Component(base) = &element.borrow().base_type {
        for request in requests(&base.root_element) {
            if !result.iter().any(|(name, _, _)| name == &request.0) {
                result.push(request);
            }
        }
    }
    result
}

pub fn resolve_contexts(doc: &Document, counters: &SymbolCounters, diag: &mut BuildDiagnostics) {
    let mut components = Vec::new();
    doc.visit_all_used_components(|component| components.push(component.clone()));
    let mut seen = std::collections::HashSet::new();
    let mut index = 0;
    while index < components.len() {
        let component = components[index].clone();
        index += 1;
        if !seen.insert(Rc::as_ptr(&component) as usize) {
            continue;
        }
        crate::object_tree::recurse_elem_no_borrow(
            &component.root_element,
            &(),
            &mut |element, _| {
                if let ElementType::Component(base) = &element.borrow().base_type {
                    components.push(base.clone());
                }
            },
        );
        components
            .extend(component.popup_windows.borrow().iter().map(|popup| popup.component.clone()));
    }
    seen.clear();
    components.retain(|component| seen.insert(Rc::as_ptr(component) as usize));
    let mut wired = std::collections::HashSet::new();
    loop {
        let mut added = false;
        for component in &components {
            crate::object_tree::recurse_elem_no_borrow(
                &component.root_element,
                &Vec::<ElementRc>::new(),
                &mut |element, ancestors| {
                    let needed = requests(element);
                    // Root requests are inputs supplied by each instance's parent.
                    if !Rc::ptr_eq(element, &component.root_element) {
                        let receiver = match &element.borrow().base_type {
                            ElementType::Component(base) if base.parent_element().is_some() => {
                                base.root_element.clone()
                            }
                            _ => element.clone(),
                        };
                        for (name, context, ty) in needed {
                            if !wired.insert((Rc::as_ptr(&receiver) as usize, name.clone())) {
                                continue;
                            }
                            let provider = ancestors.iter().rev().find_map(|ancestor| {
                                let lookup = ancestor
                                    .borrow()
                                    .lookup_property(&context, PropertyLookupMode::ComponentLocal);
                                (lookup.property_type != Type::Invalid)
                                    .then(|| (ancestor.clone(), lookup))
                            });
                            let (value, supplied) = if let Some((provider, lookup)) = provider {
                                if !lookup.property_type.can_convert(&ty) {
                                    diag.push_error(format!("Inherited property '{context}' has type {}, expected {ty}", lookup.property_type), &*element.borrow());
                                    continue;
                                }
                                let reference = Expression::PropertyReference(NamedReference::new(
                                    &provider,
                                    lookup
                                        .internal_name
                                        .unwrap_or_else(|| lookup.resolved_name.into()),
                                ));
                                (
                                    reference.maybe_convert_to(
                                        ty.clone(),
                                        &*element.borrow(),
                                        diag,
                                        counters,
                                    ),
                                    Expression::BoolLiteral(true),
                                )
                            } else {
                                let forwarding = requests(&component.root_element)
                                    .into_iter()
                                    .find(|(_, existing, existing_ty)| {
                                        existing == &context && existing_ty == &ty
                                    })
                                    .map(|(name, _, _)| name)
                                    .unwrap_or_else(|| {
                                        let name = counters.generate_name("inherited-context-");
                                        declare_request(
                                            &component.root_element,
                                            name.clone(),
                                            context.clone(),
                                            ty.clone(),
                                        );
                                        added = true;
                                        name
                                    });
                                (
                                    Expression::PropertyReference(NamedReference::new(
                                        &component.root_element,
                                        forwarding.clone(),
                                    )),
                                    Expression::PropertyReference(NamedReference::new(
                                        &component.root_element,
                                        presence(&forwarding),
                                    )),
                                )
                            };
                            let mut element = receiver.borrow_mut();
                            element.set_binding(name.clone(), value.into());
                            element.set_binding(presence(&name), supplied.into());
                        }
                    }
                    let mut children_scope = ancestors.clone();
                    children_scope.push(element.clone());
                    children_scope
                },
            );
        }
        if !added {
            break;
        }
    }
}
