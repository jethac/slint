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
