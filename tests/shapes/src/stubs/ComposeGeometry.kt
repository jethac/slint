/*
 * Minimal semantic stubs of androidx.compose.ui.geometry types used by
 * MaterialShapes.kt / ShapeUtil.kt at the pin. Value-class packing is elided;
 * the math semantics are identical (all per-component Float ops).
 */

package androidx.compose.ui.geometry

import kotlin.math.sqrt

/** Stub for `androidx.compose.ui.geometry.Offset`. */
public data class Offset(public val x: Float, public val y: Float) {
    public companion object {
        @JvmField public val Zero: Offset = Offset(0f, 0f)
        @JvmField public val Unspecified: Offset = Offset(Float.NaN, Float.NaN)
        @JvmField public val Infinite: Offset =
            Offset(Float.POSITIVE_INFINITY, Float.POSITIVE_INFINITY)
    }

    public operator fun unaryMinus(): Offset = Offset(-x, -y)
    public operator fun plus(other: Offset): Offset = Offset(x + other.x, y + other.y)
    public operator fun minus(other: Offset): Offset = Offset(x - other.x, y - other.y)
    public operator fun times(operand: Float): Offset = Offset(x * operand, y * operand)
    public operator fun div(operand: Float): Offset = Offset(x / operand, y / operand)

    /** kotlin.math.sqrt on Float widens to Double on the JVM — same as the real impl. */
    public fun getDistance(): Float = sqrt(x * x + y * y)
}

/** Stub for `androidx.compose.ui.geometry.Size`. */
public data class Size(public val width: Float, public val height: Float) {
    public companion object {
        @JvmField public val Unspecified: Size = Size(Float.NaN, Float.NaN)
    }
}

/** `androidx.compose.ui.geometry.center` extension. */
public val Size.center: Offset
    get() = Offset(width / 2f, height / 2f)

/** `Rect.center` extension used by `MaterialShapes.createOutline`. */
public val Rect.center: Offset
    get() = Offset((left + right) / 2f, (top + bottom) / 2f)

/** Stub for `androidx.compose.ui.geometry.Rect`. */
public class Rect(
    public val left: Float = 0f,
    public val top: Float = 0f,
    public val right: Float = 0f,
    public val bottom: Float = 0f,
) {
    public val width: Float get() = right - left
    public val height: Float get() = bottom - top
}

/** Stub for `androidx.compose.ui.geometry.MutableRect`. */
public class MutableRect(
    public var left: Float = 0f,
    public var top: Float = 0f,
    public var right: Float = 0f,
    public var bottom: Float = 0f,
) {
    public val width: Float get() = right - left
    public val height: Float get() = bottom - top
}
