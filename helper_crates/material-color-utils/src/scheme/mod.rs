// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Variant-specific constructors for DynamicScheme.

pub mod scheme_cmf;
pub mod scheme_content;
pub mod scheme_expressive;
pub mod scheme_fidelity;
pub mod scheme_fruit_salad;
pub mod scheme_monochrome;
pub mod scheme_neutral;
pub mod scheme_rainbow;
pub mod scheme_tonal_spot;
pub mod scheme_vibrant;

pub use scheme_cmf::SchemeCmf;
pub use scheme_content::SchemeContent;
pub use scheme_expressive::SchemeExpressive;
pub use scheme_fidelity::SchemeFidelity;
pub use scheme_fruit_salad::SchemeFruitSalad;
pub use scheme_monochrome::SchemeMonochrome;
pub use scheme_neutral::SchemeNeutral;
pub use scheme_rainbow::SchemeRainbow;
pub use scheme_tonal_spot::SchemeTonalSpot;
pub use scheme_vibrant::SchemeVibrant;
