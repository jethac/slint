/*
 * Minimal stubs of androidx.compose.ui.graphics / .unit / .util / .runtime and
 * androidx.compose.material3-internal symbols used by MaterialShapes.kt and
 * ShapeUtil.kt. Only symbols needed to compile are provided; functions whose
 * result would feed golden vectors (Matrix.map) use the real pinned source.
 */

package androidx.compose.ui.graphics

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection

/** Stub for `androidx.compose.ui.graphics.Path` (accumulates segments; unused here). */
public class Path {
    public fun rewind() {}
    public fun moveTo(x: Float, y: Float) {}
    public fun lineTo(x: Float, y: Float) {}
    public fun cubicTo(x1: Float, y1: Float, x2: Float, y2: Float, x3: Float, y3: Float) {}
    public fun close() {}
    public fun transform(matrix: Matrix) {}
    public fun addPath(path: Path) {}
    public fun translate(offset: Offset) {}
    public fun getBounds(): androidx.compose.ui.geometry.Rect =
        androidx.compose.ui.geometry.Rect()
}

/** Stub for `androidx.compose.ui.graphics.Outline`. */
public sealed class Outline {
    public class Generic(public val path: Path) : Outline()
    public class Rectangle(public val rect: androidx.compose.ui.geometry.Rect) : Outline()
    public class Rounded(public val roundRect: RoundRect) : Outline()
}

/** Stub for `androidx.compose.ui.graphics.RoundRect`. */
public class RoundRect(
    public val left: Float = 0f,
    public val top: Float = 0f,
    public val right: Float = 0f,
    public val bottom: Float = 0f,
    public val radiusX: Float = 0f,
    public val radiusY: Float = 0f,
)

/** Stub for `androidx.compose.ui.graphics.Shape`. */
public interface Shape {
    public fun createOutline(
        size: Size,
        layoutDirection: LayoutDirection,
        density: Density
    ): Outline
}
