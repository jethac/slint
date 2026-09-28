/*
 * Minimal semantic stubs of androidx.collection types, for compiling the pinned
 * graphics-shapes sources with plain kotlinc (no androidx dependencies).
 * Semantics are identical to the real implementation for the operations the
 * library uses (packing is pure bit-packing of the two floats either way).
 */

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
