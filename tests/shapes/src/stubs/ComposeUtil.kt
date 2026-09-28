// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

package androidx.compose.ui.util

/** `androidx.compose.ui.util.fastMap`. */
public inline fun <T, R> List<T>.fastMap(transform: (T) -> R): List<R> {
    val target = ArrayList<R>(size)
    forEach { target += transform(it) }
    return target
}

/** `androidx.compose.ui.util.fastForEach`. */
public inline fun <T> List<T>.fastForEach(action: (T) -> Unit) {
    for (index in indices) action(get(index))
}

/** `androidx.compose.ui.util.fastIsFinite` — same truth table as the real impl. */
public inline fun Float.fastIsFinite(): Boolean = isFinite()

/** `androidx.compose.ui.util.fastMaxOf` (4 args, as used by Matrix.map(Rect)). */
public fun fastMaxOf(a: Float, b: Float, c: Float, d: Float): Float =
    maxOf(maxOf(a, b), maxOf(c, d))

/** `androidx.compose.ui.util.fastMinOf` (4 args). */
public fun fastMinOf(a: Float, b: Float, c: Float, d: Float): Float =
    minOf(minOf(a, b), minOf(c, d))
