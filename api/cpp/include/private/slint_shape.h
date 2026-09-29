// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once
#include "private/slint_string.h"
#include "private/slint_sharedvector.h"
#include "private/slint_point.h"
#include "private/slint_models.h"
#include "private/slint_enums_internal.h"
#include "private/slint_builtin_structs_internal.h"
#include "private/slint_properties.h"

#include <memory>
#include <vector>

/// `extern "C"` entry points implemented in i-slint-core
/// (`graphics/shapes/ffi.rs`). Argument and output buffers are `void*` so the
/// `Shape` POD can live in this header.
namespace slint::cbindgen_private {
// Defined in the generated slint_internal.h.
struct Shape;
extern "C" {
void slint_shapes_polygon(const float *coords, uintptr_t coord_count, float radius, float smoothing,
                          void *out);
void slint_shapes_polygon_per_vertex(const float *coords, uintptr_t coord_count,
                                     const float *roundings, uintptr_t rounding_count, void *out);
void slint_shapes_regular_polygon(int32_t num_vertices, float radius, float smoothing, void *out);
void slint_shapes_regular_polygon_per_vertex(int32_t num_vertices, const float *roundings,
                                             uintptr_t rounding_count, void *out);
void slint_shapes_rectangle(float width, float height, const float *roundings,
                            uintptr_t rounding_count, void *out);
void slint_shapes_circle(int32_t num_vertices, void *out);
void slint_shapes_star(int32_t num_vertices_per_radius, float inner_radius, float radius,
                       float smoothing, float inner_rounding_radius, float inner_smoothing,
                       void *out);
void slint_shapes_pill(float width, float height, float smoothing, void *out);
void slint_shapes_pill_star(int32_t num_vertices_per_radius, float width, float height,
                            float inner_radius_ratio, float radius, float smoothing, void *out);
void slint_shapes_custom(const float *coords, uintptr_t coord_count, const float *roundings,
                         uintptr_t rounding_count, int32_t repetitions, float center_x,
                         float center_y, bool mirror, void *out);
void slint_shape_normalized(const void *shape, void *out);
void slint_shape_rotated(const void *shape, float degrees, void *out);
void slint_shape_scaled(const void *shape, float scale_x, float scale_y, void *out);
void slint_shape_translated(const void *shape, float dx, float dy, void *out);
void slint_shape_morph(const void *from, const void *to, float progress, void *out);
void slint_shapes_path(const SharedString *d, FillRule fill_rule, void *out);
bool slint_shape_compare_equal(const void *a, const void *b);
void slint_shape_to_svg_path(const void *shape, SharedString *out);
}
/// Value equality for the `#[repr(C)]` Shape POD (outline + fill rule);
/// morph-cache metadata (`content_hash`, `id`) is not part of the value.
/// Needed by `Property<Shape>`'s change check in generated code.
inline bool operator==(const Shape &a, const Shape &b)
{
    return slint_shape_compare_equal(&a, &b);
}
inline bool operator!=(const Shape &a, const Shape &b)
{
    return !(a == b);
}
}

namespace slint {

/// A point in a shape's unitless coordinate space. Matches `ShapePoint` in
/// i-slint-core (`#[repr(C)]` two `f32`s).
struct ShapePoint
{
    float x;
    float y;

    friend bool operator==(const ShapePoint &, const ShapePoint &) = default;
};

/// The kind of a feature in a `Shape`'s outline. Matches `ShapeFeatureKind`
/// in i-slint-core.
enum class ShapeFeatureKind {
    /// A straight edge between two corners.
    Edge = 0,
    /// A corner curving outwards.
    ConvexCorner = 1,
    /// A corner curving inwards.
    ConcaveCorner = 2,
};

/// A feature in a `Shape`'s outline: a range of cubics plus its kind.
/// `cubic_start`/`cubic_len` index into `Shape::cubics` in units of cubics.
/// Matches `ShapeFeature` in i-slint-core (`#[repr(C)]`).
struct ShapeFeature
{
    uint32_t cubic_start;
    uint32_t cubic_len;
    ShapeFeatureKind kind;

    friend bool operator==(const ShapeFeature &, const ShapeFeature &) = default;
};

/// A rounded-polygon shape value: a closed outline made of cubic Bézier
/// curves, segmented into edges and corners. Created through the functions in
/// `slint::shapes` or the `Shapes.*` builtins in .slint markup.
///
/// This is the `#[repr(C)]` payload of the `shape` type: `cubics` stores 8
/// floats per cubic (anchor0, control0, control1, anchor1).
struct Shape
{
    /// The cubic Bézier outline: 8 floats per cubic.
    const SharedVector<float> &cubics() const { return cubics_; }
    /// The feature segmentation of the outline.
    const SharedVector<ShapeFeature> &features() const { return features_; }
    /// The polygon's representative point (centroid).
    ShapePoint center() const { return center_; }
    /// The fill rule a renderer applies when filling this shape's outline.
    /// Part of the value (equality and serialization preserve it); ignored by
    /// measuring, morphing and transforms.
    cbindgen_private::FillRule fill_rule() const { return fill_rule_; }
    /// The content hash the Rust side computed at construction (morph cache
    /// key); 0 for a C++-default Shape. Only meaningful to Rust.
    uint64_t content_hash() const { return content_hash_; }
    /// The interned construction id the Rust side assigned; 0 when unset.
    /// Only meaningful to Rust.
    uint64_t id() const { return id_; }

    /// Returns a copy of this shape normalized so its bounding box fits the
    /// unit square centered on the origin.
    Shape normalized() const
    {
        Shape result;
        cbindgen_private::slint_shape_normalized(this, &result);
        return result;
    }

    /// Returns a copy of this shape rotated around its center by `degrees`.
    Shape rotated(float degrees) const
    {
        Shape result;
        cbindgen_private::slint_shape_rotated(this, degrees, &result);
        return result;
    }

    /// Returns a copy of this shape scaled by `scale_x`/`scale_y`.
    Shape scaled(float scale_x, float scale_y) const
    {
        Shape result;
        cbindgen_private::slint_shape_scaled(this, scale_x, scale_y, &result);
        return result;
    }

    /// Returns a copy of this shape translated by `dx`/`dy`.
    Shape translated(float dx, float dy) const
    {
        Shape result;
        cbindgen_private::slint_shape_translated(this, dx, dy, &result);
        return result;
    }

    /// Serializes this shape to an SVG path string.
    SharedString to_svg_path() const
    {
        SharedString result;
        cbindgen_private::slint_shape_to_svg_path(this, &result);
        return result;
    }

    friend bool operator==(const Shape &a, const Shape &b)
    {
        return cbindgen_private::slint_shape_compare_equal(&a, &b);
    }
    friend bool operator!=(const Shape &a, const Shape &b) { return !(a == b); }

private:
    // The payload is Rust-managed: `content_hash` and `id` are computed at
    // construction and keyed to the outline, so mutating the fields behind
    // them would let the morph cache return a stale match — the same hazard
    // the Rust side seals off by keeping `Shape`'s fields private. Field order
    // matches `#[repr(C)]` `Shape` in i-slint-core.
    SharedVector<float> cubics_;
    SharedVector<ShapeFeature> features_;
    ShapePoint center_ = {};
    /// Content hash computed by the Rust side at construction (morph cache
    /// key). Read and written by Rust only; always 0 for a C++-default Shape.
    uint64_t content_hash_ = 0;
    /// Interned construction id assigned by the Rust side; 0 when unset.
    uint64_t id_ = 0;
    cbindgen_private::FillRule fill_rule_ = cbindgen_private::FillRule::Nonzero;
};

/// Construction and morphing functions for \ref Shape values. These are the
/// C++ counterparts of the `Shapes.*` namespace in the .slint language.
namespace shapes {

namespace internal {

inline std::vector<float>
flatten_points(const std::shared_ptr<slint::Model<LogicalPosition>> &vertices)
{
    std::vector<float> flat;
    const size_t n = vertices ? vertices->row_count() : 0;
    flat.reserve(n * 2);
    for (size_t i = 0; i < n; ++i) {
        if (auto p = vertices->row_data(i)) {
            flat.push_back(p->x);
            flat.push_back(p->y);
        }
    }
    return flat;
}

inline std::vector<float>
flatten_roundings(const std::shared_ptr<slint::Model<language::CornerRounding>> &roundings)
{
    std::vector<float> flat;
    const size_t n = roundings ? roundings->row_count() : 0;
    flat.reserve(n * 2);
    for (size_t i = 0; i < n; ++i) {
        if (auto r = roundings->row_data(i)) {
            flat.push_back(r->radius);
            flat.push_back(r->smoothing);
        }
    }
    return flat;
}

} // namespace internal

inline Shape polygon(const std::shared_ptr<slint::Model<LogicalPosition>> &vertices,
                     language::CornerRounding rounding)
{
    auto flat = internal::flatten_points(vertices);
    Shape result;
    cbindgen_private::slint_shapes_polygon(flat.data(), flat.size(), rounding.radius,
                                           rounding.smoothing, &result);
    return result;
}

inline Shape
polygon_per_vertex(const std::shared_ptr<slint::Model<LogicalPosition>> &vertices,
                   const std::shared_ptr<slint::Model<language::CornerRounding>> &roundings)
{
    auto flat = internal::flatten_points(vertices);
    auto flat_r = internal::flatten_roundings(roundings);
    Shape result;
    cbindgen_private::slint_shapes_polygon_per_vertex(flat.data(), flat.size(), flat_r.data(),
                                                      flat_r.size(), &result);
    return result;
}

inline Shape regular_polygon(int num_vertices, language::CornerRounding rounding)
{
    Shape result;
    cbindgen_private::slint_shapes_regular_polygon(num_vertices, rounding.radius,
                                                   rounding.smoothing, &result);
    return result;
}

inline Shape
regular_polygon_per_vertex(int num_vertices,
                           const std::shared_ptr<slint::Model<language::CornerRounding>> &roundings)
{
    auto flat_r = internal::flatten_roundings(roundings);
    Shape result;
    cbindgen_private::slint_shapes_regular_polygon_per_vertex(num_vertices, flat_r.data(),
                                                              flat_r.size(), &result);
    return result;
}

inline Shape rectangle(float width, float height,
                       const std::shared_ptr<slint::Model<language::CornerRounding>> &roundings)
{
    auto flat_r = internal::flatten_roundings(roundings);
    Shape result;
    cbindgen_private::slint_shapes_rectangle(width, height, flat_r.data(), flat_r.size(), &result);
    return result;
}

inline Shape circle(int num_vertices)
{
    Shape result;
    cbindgen_private::slint_shapes_circle(num_vertices, &result);
    return result;
}

inline Shape star(int num_vertices_per_radius, float inner_radius,
                  language::CornerRounding rounding, language::CornerRounding inner_rounding)
{
    Shape result;
    cbindgen_private::slint_shapes_star(num_vertices_per_radius, inner_radius, rounding.radius,
                                        rounding.smoothing, inner_rounding.radius,
                                        inner_rounding.smoothing, &result);
    return result;
}

inline Shape pill(float width, float height, float smoothing)
{
    Shape result;
    cbindgen_private::slint_shapes_pill(width, height, smoothing, &result);
    return result;
}

inline Shape pill_star(int num_vertices_per_radius, float width, float height,
                       float inner_radius_ratio, language::CornerRounding rounding)
{
    Shape result;
    cbindgen_private::slint_shapes_pill_star(num_vertices_per_radius, width, height,
                                             inner_radius_ratio, rounding.radius,
                                             rounding.smoothing, &result);
    return result;
}

inline Shape custom(const std::shared_ptr<slint::Model<LogicalPosition>> &vertices,
                    const std::shared_ptr<slint::Model<language::CornerRounding>> &roundings,
                    int repetitions, const LogicalPosition &center, bool mirror)
{
    auto flat = internal::flatten_points(vertices);
    auto flat_r = internal::flatten_roundings(roundings);
    Shape result;
    cbindgen_private::slint_shapes_custom(flat.data(), flat.size(), flat_r.data(), flat_r.size(),
                                          repetitions, center.x, center.y, mirror, &result);
    return result;
}

/// Interpolates `from` into `to` by `progress` (0..1) with feature-aware
/// anchor matching.
inline Shape morph(const Shape &from, const Shape &to, float progress)
{
    Shape result;
    cbindgen_private::slint_shape_morph(&from, &to, progress, &result);
    return result;
}

/// Builds a shape from an SVG path description `d`. `fill_rule` selects the
/// winding rule used when the shape is filled.
inline Shape path(SharedString d, cbindgen_private::FillRule fill_rule)
{
    Shape result;
    cbindgen_private::slint_shapes_path(&d, fill_rule, &result);
    return result;
}

} // namespace shapes

namespace private_api {

/// Reinterprets the public \ref Shape wrapper as its `#[repr(C)]` FFI twin
/// (`cbindgen_private::Shape`), the storage type of `shape` properties on
/// native items. The layouts match by construction; generated code uses this
/// because a prvalue cannot be reinterpret_cast to a reference.
inline const cbindgen_private::Shape &as_cbindgen_shape(const slint::Shape &shape)
{
    return *reinterpret_cast<const cbindgen_private::Shape *>(&shape);
}

// The `set_animated_binding` helper overload for `slint::Shape` lives in
// slint_properties.h before the template's definition; only the
// `set_animated_value` specialization needs the complete type and belongs
// here. `slint::Shape` is spelled out because `private_api` pulls in
// `cbindgen_private::Shape` through `using namespace`.
template<>
inline void Property<slint::Shape>::set_animated_value(
        const slint::Shape &new_value,
        const cbindgen_private::PropertyAnimation &animation_data) const
{
    cbindgen_private::slint_property_set_animated_value_shape(&inner, &get(), &new_value,
                                                              &animation_data);
}

} // namespace private_api
} // namespace slint
