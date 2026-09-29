// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Trait object for manipulating native items without knowing their type.
//!
//! Every item, property and callback is heap-allocated in its own `Pin<Rc<…>>`.
//! The trait methods dispatch by name to the item's `rtti::BuiltinItem` metadata.

use crate::Value;
use core::pin::Pin;
use i_slint_core::graphics::Brush;
use i_slint_core::items::{ItemVTable, PropertyAnimation};
use i_slint_core::properties::InterpolatedPropertyValue;
use i_slint_core::rtti::{AnimatedBindingKind, TwoWayBindingMapping};
use i_slint_core::{Callback, Property};
use std::rc::Rc;
use vtable::VRef;

/// Erased access to a boxed native item such as `Rectangle`, `Text` or `TouchArea`.
pub trait ErasedItem {
    fn get_property(self: Pin<&Self>, name: &str) -> Result<Value, ()>;

    fn set_property(
        self: Pin<&Self>,
        name: &str,
        value: Value,
        animation: Option<PropertyAnimation>,
    ) -> Result<(), ()>;

    fn set_property_binding(
        self: Pin<&Self>,
        name: &str,
        binding: Box<dyn Fn() -> Value>,
        animation: AnimatedBindingKind,
    ) -> Result<(), ()>;

    fn prepare_property_for_two_way_binding(
        self: Pin<&Self>,
        name: &str,
    ) -> Option<Pin<Rc<Property<Value>>>>;

    fn link_property_two_way_with_map(
        self: Pin<&Self>,
        name: &str,
        other: Pin<Rc<Property<Value>>>,
        mapper: Option<Rc<dyn TwoWayBindingMapping<Value>>>,
    );

    fn call_callback(self: Pin<&Self>, name: &str, args: &[Value]) -> Result<Value, ()>;

    fn set_callback_handler(
        self: Pin<&Self>,
        name: &str,
        handler: Box<dyn Fn(&[Value]) -> Value>,
    ) -> Result<(), ()>;

    /// Write a plain (non-`Property`) field, e.g. the fields updated by layouts.
    fn set_field(self: Pin<&Self>, name: &str, value: Value) -> Result<(), ()>;

    fn as_item_ref(self: Pin<&Self>) -> Pin<VRef<'_, ItemVTable>>;
}

pub type ErasedItemRc = Pin<Rc<dyn ErasedItem>>;

/// A sub-component's own `Property<Value>`.
pub type SubComponentProperty = Pin<Rc<Property<Value>>>;

/// A sub-component's own callback, with a `Vec<Value>` argument tuple.
pub type SubComponentCallback = Pin<Rc<Callback<[Value], Value>>>;

/// Animated bindings on sub-component properties need `Value` to support
/// interpolation. Numeric and brush variants interpolate; everything else
/// snaps. Numbers interpolate in `f32` because animated numeric properties
/// are `f32`-based — an `f64` blend would leak rounding noise like
/// `99.50000001117587` instead of `99.5`.
impl InterpolatedPropertyValue for Value {
    fn interpolate(&self, target: &Self, t: f32) -> Self {
        match (self, target) {
            (Value::Number(a), Value::Number(b)) => {
                let (a, b) = (*a as f32, *b as f32);
                Value::Number((a + (b - a) * t) as f64)
            }
            (Value::Brush(a), Value::Brush(b)) => Value::Brush(Brush::interpolate(a, b, t)),
            (Value::Shape(a), Value::Shape(b)) => Value::Shape(a.interpolate(b, t)),
            (Value::Model(from), Value::Model(to)) => interpolate_font_variations(from, to, t),
            _ => target.clone(),
        }
    }

    /// A physical spring can't decompose values that don't share an animatable
    /// variant, so `0` makes such a retarget snap.
    fn channel_count(&self, target: &Self) -> usize {
        match (self, target) {
            (Value::Number(_), Value::Number(_)) => 1,
            (Value::Brush(a), Value::Brush(b)) => a.channel_count(b),
            (Value::Shape(a), Value::Shape(b)) => a.channel_count(b),
            _ => 0,
        }
    }

    fn scalar_delta(&self, target: &Self) -> f32 {
        match (self, target) {
            (Value::Shape(a), Value::Shape(b)) => a.scalar_delta(b),
            _ => 0.,
        }
    }

    fn write_channels(&self, target: &Self, out: &mut [f32]) {
        match (self, target) {
            (Value::Number(a), Value::Number(_)) => out[0] = *a as f32,
            (Value::Brush(a), Value::Brush(b)) => a.write_channels(b, out),
            (Value::Shape(a), Value::Shape(b)) => a.write_channels(b, out),
            _ => debug_assert!(false, "no channels for this pair of Value"),
        }
    }

    /// `Shape`'s channel layout is directional (0 at the start shape, the morph
    /// displacement at the target), so its start/target encodings can't fall
    /// through to the absolute [`write_channels`](Self::write_channels) defaults.
    fn write_start_channels(&self, target: &Self, out: &mut [f32]) {
        match (self, target) {
            (Value::Shape(a), Value::Shape(b)) => a.write_start_channels(b, out),
            _ => self.write_channels(target, out),
        }
    }

    fn write_target_channels(&self, start: &Self, out: &mut [f32]) {
        match (self, start) {
            (Value::Shape(a), Value::Shape(b)) => a.write_target_channels(b, out),
            _ => self.write_channels(start, out),
        }
    }

    fn rebuild_from_channels(&self, target: &Self, channels: &[f32]) -> Self {
        match (self, target) {
            (Value::Number(_), Value::Number(_)) => Value::Number(channels[0] as f64),
            (Value::Brush(a), Value::Brush(b)) => {
                Value::Brush(a.rebuild_from_channels(b, channels))
            }
            (Value::Shape(a), Value::Shape(b)) => {
                Value::Shape(a.rebuild_from_channels(b, channels))
            }
            _ => self.clone(),
        }
    }
}

/// A `[FontVariation]`-typed model animates like CSS `font-variation-settings`:
/// rows pair up by position and rows whose `tag` fields match interpolate
/// their `value`. Any other model, or a shape mismatch, animates discretely,
/// switching to the target halfway through the progress like a CSS discrete
/// animation.
fn interpolate_font_variations(
    from: &i_slint_core::model::ModelRc<Value>,
    to: &i_slint_core::model::ModelRc<Value>,
    t: f32,
) -> Value {
    use i_slint_core::model::Model as _;
    if from.row_count() != to.row_count() {
        return Value::Model(if t < 0.5 { from.clone() } else { to.clone() });
    }
    let mut rows = std::vec::Vec::with_capacity(from.row_count());
    for (from_row, to_row) in from.iter().zip(to.iter()) {
        let (Value::Struct(from_row), Value::Struct(to_row)) = (&from_row, &to_row) else {
            return Value::Model(if t < 0.5 { from.clone() } else { to.clone() });
        };
        if from_row.get_field("tag") != to_row.get_field("tag") {
            return Value::Model(if t < 0.5 { from.clone() } else { to.clone() });
        }
        let mut row = to_row.clone();
        if let (Some(Value::Number(a)), Some(Value::Number(b))) =
            (from_row.get_field("value"), to_row.get_field("value"))
        {
            let (a, b) = (*a as f32, *b as f32);
            row.set_field("value".into(), Value::Number((a + (b - a) * t) as f64));
        }
        rows.push(Value::Struct(row));
    }
    Value::Model(i_slint_core::model::ModelRc::new(i_slint_core::model::VecModel::from(rows)))
}
