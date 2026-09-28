// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

package androidx.compose.runtime

/** Stub for the @Composable annotation — no compose plugin runs here. */
@Target(
    AnnotationTarget.FUNCTION,
    AnnotationTarget.PROPERTY_GETTER,
    AnnotationTarget.TYPE,
)
@Retention(AnnotationRetention.BINARY)
public annotation class Composable

/** Stub for `remember`: without a composer, just evaluates the calculation. */
public inline fun <T> remember(vararg keys: Any?, calculation: () -> T): T = calculation()
