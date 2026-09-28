// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

package org.slint.material.parity

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.interaction.FocusInteraction
import androidx.compose.foundation.interaction.HoverInteraction
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.PressInteraction
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.LocalMinimumInteractiveComponentSize
import androidx.compose.material3.MaterialExpressiveTheme
import androidx.compose.material3.MotionScheme
import androidx.compose.material3.Text
import androidx.compose.material3.Typography
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlin.math.PI

/** Every traced element reports its bounds here; ids prefixed `text:` also
 * record text metrics through `trackText`. `track` reads `boundsInRoot`, so
 * it sees the position after `offset()` regardless of modifier order — it is
 * placed last anyway to keep that dependency obvious. */
fun Modifier.track(tracer: Tracer, id: String): Modifier =
    testTag(id).onGloballyPositioned { coords: LayoutCoordinates ->
        tracer.elementBounds[id] = coords.boundsInRoot()
        tracer.backfill(id, coords.boundsInRoot())
    }

/** Text node: track bounds (mask + numeric compare) and text-layout metrics
 * (baseline, line count — the metric layer the comparator checks). */
fun Modifier.trackText(tracer: Tracer, id: String, density: Float): Modifier =
    testTag(id).onGloballyPositioned { coords: LayoutCoordinates ->
        val b = coords.boundsInRoot()
        val old = tracer.textMetrics[id] ?: TextMetric(0.0, 0.0, 0.0, 0.0, 0.0, 0, 0.0, 0.0, 0)
        tracer.textMetrics[id] = old.copy(
            x = (b.left / density).toDouble(),
            y = (b.top / density).toDouble(),
            w = (b.width / density).toDouble(),
            h = (b.height / density).toDouble(),
        )
    }

/** `onTextLayout` sink for `trackText` ids: fills in baseline, line count,
 * `frac_w` — the fractional advance of the laid-out text — and `unhint_w`,
 * an unhinted `Paint` measure of the same string at the same size and
 * weight. The laid-out width is density-hinted (it differs between 1x and
 * 2x); the unhinted measure is the quantity Slint's layout ceils into its
 * element width, and is density-independent. */
fun recordTextLayout(
    tracer: Tracer,
    id: String,
    density: androidx.compose.ui.unit.Density,
    resolver: androidx.compose.ui.text.font.FontFamily.Resolver,
    fontFamily: androidx.compose.ui.text.font.FontFamily?,
): (androidx.compose.ui.text.TextLayoutResult) -> Unit =
    { layout ->
        val style = layout.layoutInput.style
        val typeface = resolver.resolve(
            style.fontFamily
                ?: fontFamily
                ?: androidx.compose.ui.text.font.FontFamily.Default,
            style.fontWeight ?: androidx.compose.ui.text.font.FontWeight.Normal,
            style.fontStyle ?: androidx.compose.ui.text.font.FontStyle.Normal,
        ).value as? android.graphics.Typeface
        // `measureText` is still pixel-quantized under layoutlib even with
        // `HINTING_OFF` — a plain measure at 1x returns whole pixels. Measuring
        // at 8x and scaling back shrinks the quantization error to ~0.1px,
        // which is what the comparator's ceil-consistency check needs.
        val paint = android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG).apply {
            this.typeface = typeface
            textSize = with(density) { style.fontSize.toPx() } * 8f
            if (style.letterSpacing.isSp) {
                letterSpacing =
                    (with(density) { style.letterSpacing.toPx() } / textSize * 8f).toFloat()
            }
            hinting = android.graphics.Paint.HINTING_OFF
        }
        val old =
            tracer.textMetrics[id] ?: TextMetric(0.0, 0.0, 0.0, 0.0, 0.0, 0, 0.0, 0.0, 0)
        tracer.textMetrics[id] = old.copy(
            baseline = layout.firstBaseline.toDouble() / density.density,
            lines = layout.lineCount,
            // The real fractional advance of the laid-out text, in dp like
            // every other metric — `boundsInRoot`'s `w` is the layout
            // node's rounded width, these are the engine's own subpixel
            // line edges.
            fracW = (layout.getLineRight(0) - layout.getLineLeft(0)).toDouble() /
                density.density,
            unhintW = paint.measureText(layout.layoutInput.text.text).toDouble() /
                (density.density * 8.0),
            chars = layout.layoutInput.text.text.length,
        )
    }

/** Re-derives the scene's scheme through the vendored official
 * `material-color-utilities` Java sources (pinned commit, see
 * vendor/mcu/VERSION.md) — an implementation independent of Slint's Rust
 * port, so a bug in either shows up here. The generator's resolved
 * `scheme` map (Slint's port output) is compared against it by
 * [assertSchemeMatches]. */
fun mcuScheme(scene: Scene): dynamiccolor.DynamicScheme {
    val t = scene.theme
    val seed = hct.Hct.fromInt(
        (0xFF00_0000L or t.getString("seed").toLong(16)).toInt(),
    )
    val dark = t.optBoolean("dark", false)
    val contrast = t.optDouble("contrast", 0.0)
    val spec = when (val s = t.optString("spec", "spec2025")) {
        "spec2021" -> dynamiccolor.ColorSpec.SpecVersion.SPEC_2021
        "spec2025" -> dynamiccolor.ColorSpec.SpecVersion.SPEC_2025
        else -> error("unknown spec $s")
    }
    val platform = when (val p = t.optString("platform", "phone")) {
        "phone" -> dynamiccolor.DynamicScheme.Platform.PHONE
        "watch" -> dynamiccolor.DynamicScheme.Platform.WATCH
        else -> error("unknown platform $p")
    }
    return when (val v = t.getString("variant")) {
        "tonal-spot" -> scheme.SchemeTonalSpot(seed, dark, contrast, spec, platform)
        "expressive" -> scheme.SchemeExpressive(seed, dark, contrast, spec, platform)
        else -> error("mcuScheme has no mapping for variant $v")
    }
}

/** Every `ColorScheme` role the generator resolves, read off the
 * official DynamicScheme. */
private fun roleArgb(s: dynamiccolor.DynamicScheme, role: String): Int = when (role) {
    "primary" -> s.primary
    "onPrimary" -> s.onPrimary
    "primaryContainer" -> s.primaryContainer
    "onPrimaryContainer" -> s.onPrimaryContainer
    "inversePrimary" -> s.inversePrimary
    "secondary" -> s.secondary
    "onSecondary" -> s.onSecondary
    "secondaryContainer" -> s.secondaryContainer
    "onSecondaryContainer" -> s.onSecondaryContainer
    "tertiary" -> s.tertiary
    "onTertiary" -> s.onTertiary
    "tertiaryContainer" -> s.tertiaryContainer
    "onTertiaryContainer" -> s.onTertiaryContainer
    "background" -> s.background
    "onBackground" -> s.onBackground
    "surface" -> s.surface
    "onSurface" -> s.onSurface
    "surfaceVariant" -> s.surfaceVariant
    "onSurfaceVariant" -> s.onSurfaceVariant
    "surfaceTint" -> s.surfaceTint
    "inverseSurface" -> s.inverseSurface
    "inverseOnSurface" -> s.inverseOnSurface
    "error" -> s.error
    "onError" -> s.onError
    "errorContainer" -> s.errorContainer
    "onErrorContainer" -> s.onErrorContainer
    "outline" -> s.outline
    "outlineVariant" -> s.outlineVariant
    "scrim" -> s.scrim
    "surfaceBright" -> s.surfaceBright
    "surfaceDim" -> s.surfaceDim
    "surfaceContainer" -> s.surfaceContainer
    "surfaceContainerHigh" -> s.surfaceContainerHigh
    "surfaceContainerHighest" -> s.surfaceContainerHighest
    "surfaceContainerLow" -> s.surfaceContainerLow
    "surfaceContainerLowest" -> s.surfaceContainerLowest
    "primaryFixed" -> s.primaryFixed
    "primaryFixedDim" -> s.primaryFixedDim
    "onPrimaryFixed" -> s.onPrimaryFixed
    "onPrimaryFixedVariant" -> s.onPrimaryFixedVariant
    "secondaryFixed" -> s.secondaryFixed
    "secondaryFixedDim" -> s.secondaryFixedDim
    "onSecondaryFixed" -> s.onSecondaryFixed
    "onSecondaryFixedVariant" -> s.onSecondaryFixedVariant
    "tertiaryFixed" -> s.tertiaryFixed
    "tertiaryFixedDim" -> s.tertiaryFixedDim
    "onTertiaryFixed" -> s.onTertiaryFixed
    "onTertiaryFixedVariant" -> s.onTertiaryFixedVariant
    else -> error("no DynamicScheme mapping for scheme role $role")
}

/** The `ColorScheme` the scene renders: the independently derived scheme —
 * Slint's theme resolution runs its Rust port, this runs the official Java
 * implementation on the same seed/variant/spec/platform/dark/contrast
 * inputs. */
fun sceneColorScheme(scene: Scene): androidx.compose.material3.ColorScheme {
    val s = mcuScheme(scene)
    return lightColorScheme(
        primary = Color(roleArgb(s, "primary")),
        onPrimary = Color(roleArgb(s, "onPrimary")),
        primaryContainer = Color(roleArgb(s, "primaryContainer")),
        onPrimaryContainer = Color(roleArgb(s, "onPrimaryContainer")),
        inversePrimary = Color(roleArgb(s, "inversePrimary")),
        secondary = Color(roleArgb(s, "secondary")),
        onSecondary = Color(roleArgb(s, "onSecondary")),
        secondaryContainer = Color(roleArgb(s, "secondaryContainer")),
        onSecondaryContainer = Color(roleArgb(s, "onSecondaryContainer")),
        tertiary = Color(roleArgb(s, "tertiary")),
        onTertiary = Color(roleArgb(s, "onTertiary")),
        tertiaryContainer = Color(roleArgb(s, "tertiaryContainer")),
        onTertiaryContainer = Color(roleArgb(s, "onTertiaryContainer")),
        background = Color(roleArgb(s, "background")),
        onBackground = Color(roleArgb(s, "onBackground")),
        surface = Color(roleArgb(s, "surface")),
        onSurface = Color(roleArgb(s, "onSurface")),
        surfaceVariant = Color(roleArgb(s, "surfaceVariant")),
        onSurfaceVariant = Color(roleArgb(s, "onSurfaceVariant")),
        surfaceTint = Color(roleArgb(s, "surfaceTint")),
        inverseSurface = Color(roleArgb(s, "inverseSurface")),
        inverseOnSurface = Color(roleArgb(s, "inverseOnSurface")),
        error = Color(roleArgb(s, "error")),
        onError = Color(roleArgb(s, "onError")),
        errorContainer = Color(roleArgb(s, "errorContainer")),
        onErrorContainer = Color(roleArgb(s, "onErrorContainer")),
        outline = Color(roleArgb(s, "outline")),
        outlineVariant = Color(roleArgb(s, "outlineVariant")),
        scrim = Color(roleArgb(s, "scrim")),
        surfaceBright = Color(roleArgb(s, "surfaceBright")),
        surfaceDim = Color(roleArgb(s, "surfaceDim")),
        surfaceContainer = Color(roleArgb(s, "surfaceContainer")),
        surfaceContainerHigh = Color(roleArgb(s, "surfaceContainerHigh")),
        surfaceContainerHighest = Color(roleArgb(s, "surfaceContainerHighest")),
        surfaceContainerLow = Color(roleArgb(s, "surfaceContainerLow")),
        surfaceContainerLowest = Color(roleArgb(s, "surfaceContainerLowest")),
        primaryFixed = Color(roleArgb(s, "primaryFixed")),
        primaryFixedDim = Color(roleArgb(s, "primaryFixedDim")),
        onPrimaryFixed = Color(roleArgb(s, "onPrimaryFixed")),
        onPrimaryFixedVariant = Color(roleArgb(s, "onPrimaryFixedVariant")),
        secondaryFixed = Color(roleArgb(s, "secondaryFixed")),
        secondaryFixedDim = Color(roleArgb(s, "secondaryFixedDim")),
        onSecondaryFixed = Color(roleArgb(s, "onSecondaryFixed")),
        onSecondaryFixedVariant = Color(roleArgb(s, "onSecondaryFixedVariant")),
        tertiaryFixed = Color(roleArgb(s, "tertiaryFixed")),
        tertiaryFixedDim = Color(roleArgb(s, "tertiaryFixedDim")),
        onTertiaryFixed = Color(roleArgb(s, "onTertiaryFixed")),
        onTertiaryFixedVariant = Color(roleArgb(s, "onTertiaryFixedVariant")),
    )
}

/** Fails when the generator's resolved scheme (Slint's `material-color-utils`
 * port, applied on the Slint side) disagrees with the materialkolor port —
 * the cross-implementation check that keeps the reference independent of the
 * code under test. */
fun assertSchemeMatches(scene: Scene) {
    val s = mcuScheme(scene)
    val mismatches = scene.scheme.mapNotNull { (role, expected) ->
        val actual = roleArgb(s, role.kebabToCamel())
        when {
            expected != (actual.toLong() and 0xFFFF_FFFFL) ->
                "$role: scene ${expected.toString(16)} vs compose ${(actual.toLong() and 0xFFFF_FFFFL).toString(16)}"
            else -> null
        }
    }
    check(mismatches.isEmpty()) {
        "Slint-generated scheme differs from the materialkolor scheme:\n${mismatches.joinToString("\n")}"
    }
}

/** Material typography: the default `Typography()` with only the font family
 * replaced, so sizes, weights, line heights and tracking stay upstream. */
fun typography(font: FontFamily?): Typography =
    Typography().let { t ->
        t.copy(
            displayLarge = t.displayLarge.copy(fontFamily = font),
            displayMedium = t.displayMedium.copy(fontFamily = font),
            displaySmall = t.displaySmall.copy(fontFamily = font),
            headlineLarge = t.headlineLarge.copy(fontFamily = font),
            headlineMedium = t.headlineMedium.copy(fontFamily = font),
            headlineSmall = t.headlineSmall.copy(fontFamily = font),
            titleLarge = t.titleLarge.copy(fontFamily = font),
            titleMedium = t.titleMedium.copy(fontFamily = font),
            titleSmall = t.titleSmall.copy(fontFamily = font),
            bodyLarge = t.bodyLarge.copy(fontFamily = font),
            bodyMedium = t.bodyMedium.copy(fontFamily = font),
            bodySmall = t.bodySmall.copy(fontFamily = font),
            labelLarge = t.labelLarge.copy(fontFamily = font),
            labelMedium = t.labelMedium.copy(fontFamily = font),
            labelSmall = t.labelSmall.copy(fontFamily = font),
        )
    }

/** The composable a scene renders, keyed on its `type`. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class, ExperimentalMaterial3Api::class)
@Composable
fun SceneContent(scene: Scene, font: FontFamily?, tracer: Tracer) {
    MaterialExpressiveTheme(
        colorScheme = sceneColorScheme(scene),
        motionScheme = MotionScheme.expressive(),
        typography = typography(font),
    ) {
        // Upstream pads components out to a 48dp touch-target slot and centers
        // the visual in it; the scene coordinates place the drawn component,
        // so that padding is off here — the Slint library draws the same
        // visual at the same declared bounds.
        CompositionLocalProvider(LocalMinimumInteractiveComponentSize provides 0.dp) {
            FrameRecorder(scene, tracer)
            when (scene.type) {
                "canvas" -> CanvasScene(scene, tracer)
                "spring-motion" -> SpringMotionScene(scene, tracer)
                else -> error("unknown scene type ${scene.type}")
            }
        }
    }
}

/** Records one trace frame per Composable frame tick — under Paparazzi the
 * frame clock advances in exact `1/fps` steps, so `tNanos` indexes frames. */
@Composable
private fun FrameRecorder(scene: Scene, tracer: Tracer) {
    val density = androidx.compose.ui.platform.LocalDensity.current.density
    LaunchedEffect(scene.name) {
        while (true) {
            withFrameNanos { nanos ->
                tracer.recordFrame(nanos / 1_000_000, scene.traceProps, scene.traceElements, density)
            }
        }
    }
}

@Composable
private fun CanvasScene(scene: Scene, tracer: Tracer) {
    val (w, h) = scene.sizeDp
    val density = androidx.compose.ui.platform.LocalDensity.current.density
    val scheme = androidx.compose.material3.MaterialTheme.colorScheme
    Box(
        Modifier.testTag("scene-root").size(w.dp, h.dp).background(scheme.background),
    ) {
        scene.widgets.forEachIndexed { i, widget ->
            when (widget.kind) {
                "filled-button" -> StateButton(widget, tracer, "text:$i", "button$i", density)
                "rect" ->
                    Box(
                        Modifier.offset(widget.x.dp, widget.y.dp)
                            .size(widget.width.dp, widget.height.dp)
                            .clip(RoundedCornerShapeOrRect(widget.radius.dp))
                            .background(schemeColor(widget.color ?: "primary")),
                    )
                else -> error("unknown widget kind ${widget.kind}")
            }
        }
    }
}

/** A filled button in the interaction state the scene asks for. `state`
 * comes from the scene's `widgets[].state` — the Slint side drives the same
 * state through real pointer events on the mocked backend. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun StateButton(widget: Widget, tracer: Tracer, textId: String, elementId: String, density: Float) {
    val interactionSource = remember { MutableInteractionSource() }
    // Mirrors the Slint driver's event stream: a held pointer produces
    // hover AND press, so "pressed" emits Enter then Press. The press
    // position matches the driver's press point (40,20)dp into the widget.
    val pressPos = with(androidx.compose.ui.platform.LocalDensity.current) {
        Offset(40.dp.toPx(), 20.dp.toPx())
    }
    when (widget.state) {
        "pressed" -> LaunchedEffect(Unit) {
            // Emit on the first frame at t >= 1 ms: a press arriving at t = 0
            // makes the ripple request a frame at t = 0, which aborts
            // layoutlib natively under `gif()`.
            var emitted = false
            while (!emitted) {
                withFrameNanos { nanos ->
                    if (nanos >= 1_000_000 && !emitted) {
                        interactionSource.tryEmit(HoverInteraction.Enter())
                        interactionSource.tryEmit(PressInteraction.Press(pressPos))
                        emitted = true
                    }
                }
            }
        }
        "hovered" -> LaunchedEffect(Unit) {
            interactionSource.emit(HoverInteraction.Enter())
        }
        "focused" -> LaunchedEffect(Unit) {
            interactionSource.emit(FocusInteraction.Focus())
        }
    }
    Button(
        onClick = {},
        enabled = widget.enabled,
        modifier = Modifier.offset(widget.x.dp, widget.y.dp).track(tracer, elementId),
        // The Expressive button API: the base shape morphs into
        // `ButtonShapes.pressedShape` while pressed, matching the Slint
        // side's pressed-state shape.
        shapes = ButtonDefaults.shapes(),
        interactionSource = interactionSource,
    ) {
        Text(
            widget.text ?: "",
            modifier = Modifier.trackText(tracer, textId, density),
            onTextLayout = recordTextLayout(
                tracer,
                textId,
                LocalDensity.current,
                androidx.compose.ui.platform.LocalFontFamilyResolver.current,
                androidx.compose.material3.LocalTextStyle.current.fontFamily,
            ),
        )
    }
}

private fun RoundedCornerShapeOrRect(radius: Dp): Shape =
    if (radius <= 0.dp) RectangleShape else androidx.compose.foundation.shape.RoundedCornerShape(radius)

@Composable
private fun schemeColor(role: String): Color =
    androidx.compose.material3.MaterialTheme.colorScheme.let { scheme ->
        when (role.kebabToCamel()) {
            "primary" -> scheme.primary
            "primaryContainer" -> scheme.primaryContainer
            "secondary" -> scheme.secondary
            "secondaryContainer" -> scheme.secondaryContainer
            "tertiary" -> scheme.tertiary
            "tertiaryContainer" -> scheme.tertiaryContainer
            "surface" -> scheme.surface
            "background" -> scheme.background
            "error" -> scheme.error
            "errorContainer" -> scheme.errorContainer
            else -> error("scene catalog has no ColorScheme role for $role")
        }
    }

@Composable
private fun SpringMotionScene(scene: Scene, tracer: Tracer) {
    val p = scene.params
    val start = p.getDouble("start").toFloat()
    val target = p.getDouble("target").toFloat()
    val y = p.getDouble("y").toFloat()
    val size = p.getDouble("size").toFloat()
    val radius = p.getDouble("radius").toFloat()
    val durationMs = p.getDouble("duration_ms")
    val bounce = p.getDouble("bounce")
    val (w, h) = scene.sizeDp

    // Slint's `easing: spring(bounce)` with `duration` is a damped harmonic
    // oscillator with `w_n = 2*pi/duration` and `zeta = 1 - bounce`
    // (internal/core/animations/simulations/spring.rs). Compose's `spring()`
    // uses `stiffness = w_n^2` and `dampingRatio = zeta` (mass 1) — the same
    // ODE, so the traces compare directly.
    val wn = 2.0 * PI / (durationMs / 1000.0)
    var goal by remember { mutableStateOf(start) }
    val x by animateDpAsState(
        targetValue = goal.dp,
        animationSpec = spring(
            dampingRatio = (1.0 - bounce).toFloat(),
            stiffness = (wn * wn).toFloat(),
        ),
        label = "thumb-x",
    )

    tracer.propGetters["thumb-x"] = { x.value.toDouble() }
    tracer.elementOpacity["thumb"] = 1f

    // The scene's `actions` describe user input at t=0; under Paparazzi there
    // is no input injection, so a `press`/`release` pair becomes "the gesture
    // fires when composition starts". The Slint side delivers real pointer
    // events on the mocked backend.
    if (scene.actions.isNotEmpty()) {
        LaunchedEffect(Unit) { goal = target }
    }

    val scheme = androidx.compose.material3.MaterialTheme.colorScheme
    Box(
        Modifier.testTag("scene-root").size(w.dp, h.dp).background(scheme.background),
    ) {
        Box(
            Modifier.offset(x, y.dp)
                .size(size.dp)
                .clip(RoundedCornerShapeOrRect(radius.dp))
                .background(schemeColor(p.getString("color")))
                .track(tracer, "thumb"),
        )
    }
}
