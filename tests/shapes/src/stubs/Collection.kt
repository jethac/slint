// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

package androidx.collection

/** Stub for `androidx.collection.FloatFloatPair` (a value class packing two floats). */
public class FloatFloatPair(public val first: Float, public val second: Float) {
    public operator fun component1(): Float = first
    public operator fun component2(): Float = second

    override fun equals(other: Any?): Boolean =
        other is FloatFloatPair && first == other.first && second == other.second
    override fun hashCode(): Int = first.hashCode() * 31 + second.hashCode()
    override fun toString(): String = "($first, $second)"
}

/**
 * Stub for `androidx.collection.FloatList`/`MutableFloatList`.
 * The library only uses `size`, `get`, `indices`, iteration and `add`.
 */
public typealias MutableFloatList = java.util.ArrayList<Float>
public typealias FloatList = java.util.ArrayList<Float>
