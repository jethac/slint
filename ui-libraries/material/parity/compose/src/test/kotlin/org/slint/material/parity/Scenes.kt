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
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ElevatedButton
import androidx.compose.material3.ElevatedToggleButton
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.FilledIconToggleButton
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.FilledTonalIconToggleButton
import androidx.compose.material3.TonalToggleButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.IconButtonDefaults.IconButtonWidthOption
import androidx.compose.material3.IconToggleButton
import androidx.compose.material3.LocalMinimumInteractiveComponentSize
import androidx.compose.material3.LocalRippleThemeConfiguration
import androidx.compose.material3.RippleDefaults
import androidx.compose.material3.SplitButtonDefaults
import androidx.compose.material3.SplitButtonLayout
import androidx.compose.material3.MaterialExpressiveTheme
import androidx.compose.material3.MotionScheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedIconButton
import androidx.compose.material3.OutlinedIconToggleButton
import androidx.compose.material3.OutlinedToggleButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.ToggleButton
import androidx.compose.material3.ToggleButtonDefaults
import androidx.compose.material3.Typography
import androidx.compose.material3.lightColorScheme
import androidx.compose.material3.toShape
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.CenterAlignedTopAppBar
import androidx.compose.material3.MediumTopAppBar
import androidx.compose.material3.MediumFlexibleTopAppBar
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.LargeFlexibleTopAppBar
import androidx.compose.material3.TwoRowsTopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberTopAppBarState
import androidx.compose.material3.BottomAppBar
import androidx.compose.material3.BottomAppBarDefaults
import androidx.compose.material3.rememberBottomAppBarState
import androidx.compose.material3.SearchBar
import androidx.compose.material3.SearchBarDefaults
import androidx.compose.material3.AppBarWithSearch
import androidx.compose.material3.rememberSearchBarState
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.requiredHeight
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.gestures.AnchoredDraggableDefaults
import androidx.compose.foundation.gestures.AnchoredDraggableState
import androidx.compose.foundation.gestures.DraggableAnchors
import androidx.compose.foundation.gestures.FlingBehavior
import androidx.compose.foundation.gestures.ScrollScope
import androidx.compose.foundation.interaction.DragInteraction
import androidx.compose.foundation.interaction.collectIsDraggedAsState
import androidx.compose.material3.BottomSheet
import androidx.compose.material3.BottomSheetDefaults
import androidx.compose.material3.BottomSheetScaffold
import androidx.compose.material3.SheetValue
import androidx.compose.material3.VerticalDragHandle
import androidx.compose.material3.SheetState
import androidx.compose.material3.rememberBottomSheetScaffoldState
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.material3.rememberStandardBottomSheetState
import androidx.compose.foundation.gestures.animateTo
import androidx.compose.ui.Alignment
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.platform.LocalViewConfiguration
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.rememberCoroutineScope
import kotlin.math.roundToInt
import kotlinx.coroutines.launch
import androidx.compose.ui.graphics.vector.path
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
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

/** Sheet surface: track bounds like [track] and also register
 * `sheet-pill{i}` — the drag-handle pill's bounds derived from the surface
 * (the real BottomSheet draws its own handle): 32x4 dp centered, 22 dp
 * below the surface top (`SheetDefaults.kt`'s DragHandleVerticalPadding). */
fun Modifier.trackSheet(
    tracer: Tracer,
    id: String,
    uiDensity: androidx.compose.ui.unit.Density,
): Modifier =
    testTag(id).onGloballyPositioned { coords: LayoutCoordinates ->
        val b = coords.boundsInRoot()
        tracer.elementBounds[id] = b
        tracer.backfill(id, b)
        val pill = "sheet-pill" + id.filter(Char::isDigit)
        val w32 = with(uiDensity) { 32.dp.toPx() }
        val h4 = with(uiDensity) { 4.dp.toPx() }
        val v22 = with(uiDensity) { 22.dp.toPx() }
        val pb = Rect(
            b.left + (b.width - w32) / 2f,
            b.top + v22,
            b.left + (b.width - w32) / 2f + w32,
            b.top + v22 + h4,
        )
        tracer.elementBounds[pill] = pb
        tracer.backfill(pill, pb)
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
fun SceneContent(
    scene: Scene,
    font: FontFamily?,
    tracer: Tracer,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
) {
    MaterialExpressiveTheme(
        colorScheme = sceneColorScheme(scene),
        motionScheme = MotionScheme.expressive(),
        typography = typography(font),
    ) {
        // Upstream pads components out to a 48dp touch-target slot and centers
        // the visual in it; the scene coordinates place the drawn component,
        // so that padding is off here — the Slint library draws the same
        // visual at the same declared bounds.
        CompositionLocalProvider(
            LocalMinimumInteractiveComponentSize provides 0.dp,
            // The issue pins the M3 inset focus ring (two strokes following
            // the container shape); upstream ships it as an opt-in ripple
            // theme, off by default.
            LocalRippleThemeConfiguration provides
                RippleDefaults.InsetFocusRingRippleThemeConfiguration,
        ) {
            when (scene.type) {
                "canvas" -> CanvasScene(scene, tracer, emitPress)
                "spring-motion" -> SpringMotionScene(scene, tracer)
                else -> error("unknown scene type ${scene.type}")
            }
        }
    }
}

// Trace frames are recorded from the snapshot handler after each drawn
// frame (see RenderTest) — post-draw sampling keeps `trace@N` aligned with
// the pixels of `frame_N`, which pre-draw `withFrameNanos` sampling cannot
// (it records the state drawn in the previous frame).
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun CanvasScene(
    scene: Scene,
    tracer: Tracer,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
) {
    val (w, h) = scene.sizeDp
    val density = androidx.compose.ui.platform.LocalDensity.current.density
    val scheme = androidx.compose.material3.MaterialTheme.colorScheme
    Box(
        Modifier.testTag("scene-root").size(w.dp, h.dp).background(scheme.background),
    ) {
        // Elements are named `button{n}`/`text:{n}` by count of
        // button-family widgets, not widget index — a backdrop `rect`
        // ahead of a button leaves `button0` intact.
        var buttons = 0
        var surfaces = 0
        var appbars = 0
        var sheets = 0
        var vhandles = 0
        scene.widgets.forEach { widget ->
            when {
                widget.isIconButton -> StateIconButton(
                    widget,
                    scene,
                    tracer,
                    "button${buttons++}",
                    density,
                    emitPress,
                )
                widget.isSplitButton -> StateSplitButton(
                    widget,
                    scene,
                    tracer,
                    "text:${buttons}",
                    "button${buttons++}",
                    density,
                    emitPress,
                )
                widget.isButton -> StateButton(
                    widget,
                    scene,
                    tracer,
                    "text:${buttons}",
                    "button${buttons++}",
                    density,
                    emitPress,
                )
                widget.kind == "top-app-bar" ||
                    widget.kind == "bottom-app-bar" ||
                    widget.kind == "search-bar" ||
                    widget.kind == "app-bar-with-search" ->
                    StateAppBar(widget, tracer, "appbar${appbars++}")
                widget.kind == "rect" ->
                    Box(
                        Modifier.offset(widget.x.dp, widget.y.dp)
                            .size(widget.width.dp, widget.height.dp)
                            .clip(RoundedCornerShapeOrRect(widget.radius.dp))
                            .background(schemeColor(widget.color ?: "primary")),
                    )
                widget.kind == "elevated-rect" ->
                    Box(
                        Modifier.offset(widget.x.dp, widget.y.dp)
                            .size(widget.width.dp, widget.height.dp)
                            // Modifier.shadow draws the real Android
                            // ambient+spot shadow for the shape.
                            .shadow(
                                widget.elevation.dp,
                                RoundedCornerShapeOrRect(widget.radius.dp),
                            )
                            .background(
                                schemeColor(widget.color ?: "primary"),
                                RoundedCornerShapeOrRect(widget.radius.dp),
                            ),
                    )
                widget.kind == "surface" -> {
                    // A clip + color surface: the shape machinery's outline
                    // and fills. Platform shadows (`Modifier.shadow`,
                    // `View.elevation`) deadlock layoutlib's hardware
                    // renderer — `nSyncAndDrawFrame` never returns — so the
                    // parity surfaces are unelevated; the shadow recipe
                    // itself is validated against Skia's native `draw_shadow`
                    // in the i-slint-renderer-skia tests.
                    val shape = widget.outline()
                    val tag = "surface${surfaces++}"
                    Box(
                        Modifier.offset(widget.x.dp, widget.y.dp)
                            .size(widget.width.dp, widget.height.dp)
                            .clip(shape)
                            .background(schemeColor(widget.color ?: "surface"))
                            .track(tracer, tag),
                    )
                }
                widget.kind == "drag-handle" ->
                    // SheetDefaults.kt's DragHandle: a 48dp-tall strip with
                    // the 32x4 pill centered — the Slint BottomSheetDragHandle
                    // draws the same strip.
                    Box(
                        Modifier.offset(widget.x.dp, widget.y.dp)
                            .size(widget.width.dp, 48.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        BottomSheetDefaults.DragHandle()
                    }
                widget.kind == "vertical-drag-handle" ->
                    StateVerticalDragHandle(
                        widget, scene, tracer, emitPress, "vhandle${vhandles++}")
                widget.kind == "bottom-sheet" ||
                    widget.kind == "bottom-sheet-scaffold" ||
                    widget.kind == "modal-bottom-sheet" -> {
                    // `modal{n}` matches the Slint element id the generator
                    // emits for the popup.
                    val tag =
                        if (widget.kind == "modal-bottom-sheet")
                            "modal${sheets++}" else "sheet${sheets++}"
                    if (sheetNeedsGestureReplay(widget, scene)) {
                        ParitySheet(widget, scene, tracer, emitPress, tag)
                    } else {
                        SheetWidget(widget, scene, tracer, emitPress, tag)
                    }
                }
                else -> error("unknown widget kind ${widget.kind}")
            }
        }
    }
}

/** Container height per size bucket (dp) — `ButtonDefaults` `*ContainerHeight`. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
private fun buttonHeight(widget: Widget) = when (widget.size) {
    "xs" -> ButtonDefaults.ExtraSmallContainerHeight
    "s" -> ButtonDefaults.MinHeight
    "m" -> ButtonDefaults.MediumContainerHeight
    "l" -> ButtonDefaults.LargeContainerHeight
    "xl" -> ButtonDefaults.ExtraLargeContainerHeight
    else -> error("unknown button size ${widget.size}")
}

/** The size bucket's `Button*Tokens.ContainerShapeSquare` — exposed through
 * `ToggleButtonDefaults`' per-size getters (the same token objects back the
 * plain-button square variant upstream). */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun squareShapeFor(widget: Widget): Shape {
    val d = ToggleButtonDefaults
    return when (widget.size) {
        "xs" -> d.extraSmallSquareShape
        "s" -> d.squareShape
        "m" -> d.mediumSquareShape
        "l" -> d.largeSquareShape
        "xl" -> d.extraLargeSquareShape
        else -> error("unknown button size ${widget.size}")
    }
}

/** The token-backed `RoundedCornerShape` corner radius in dp — covers both
 * percentage (`corner_full` stadium) and fixed-dp token corners. `h` is the
 * container height: it is always the smaller dimension in these scenes, so
 * `Size(h, h)` reproduces `PercentCornerSize`'s `minDimension` exactly. */
private fun radiusOf(shape: Shape, h: androidx.compose.ui.unit.Dp, density: Float): Float {
    require(shape is androidx.compose.foundation.shape.RoundedCornerShape) {
        "token shapes are RoundedCornerShape, got $shape"
    }
    // `toPx` takes pixel sizes: percent corners resolve against the box's px
    // edge, dp corners convert via the density — either way the result is px.
    val boxPx = h.value * density
    return shape.topStart.toPx(
        androidx.compose.ui.geometry.Size(boxPx, boxPx),
        androidx.compose.ui.unit.Density(density),
    ) / density
}

/** The corner radius the container springs toward, in dp — the Slint side
 * exposes it as `container_radius`; this is the same value computed from
 * the shapes this side hands the composable. */
private fun radiusTarget(h: Float, resting: Shape, pressed: Shape, checked: Shape,
    isPressed: Boolean, isChecked: Boolean, density: Float): Float {
    val shape = when {
        isPressed -> pressed
        isChecked -> checked
        else -> resting
    }
    return radiusOf(shape, androidx.compose.ui.unit.Dp(h), density)
}

/** A filled button in the interaction state the scene asks for. `state`
 * comes from the scene's `widgets[].state` — the Slint side drives the same
 * state through real pointer events on the mocked backend. */
/** Interaction source that replays its latest emission to late collectors —
 * the button's collector may subscribe frames after the press is emitted. */
class ReplayableInteractionSource : MutableInteractionSource {
    val flow = kotlinx.coroutines.flow.MutableSharedFlow<androidx.compose.foundation.interaction.Interaction>(
        replay = 1,
        extraBufferCapacity = 16,
    )

    override val interactions get() = flow

    override suspend fun emit(interaction: androidx.compose.foundation.interaction.Interaction) {
        flow.emit(interaction)
    }

    override fun tryEmit(interaction: androidx.compose.foundation.interaction.Interaction): Boolean =
        flow.tryEmit(interaction)
}

/** Drives the widget's `state` on the interaction source — hover/focus via
 * a launched emission, press/press+release via the frame sink's post-frame-0
 * hook so it lands at the same moment the Slint driver dispatches
 * `//ACTION=`. */
@Composable
private fun emitStateInteractions(
    widget: Widget,
    scene: Scene,
    tracer: Tracer,
    elementId: String,
    interactionSource: ReplayableInteractionSource,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
    pressOffset: Offset,
    density: Float,
) {
    when (widget.state) {
        "hovered" -> LaunchedEffect(Unit) {
            interactionSource.emit(HoverInteraction.Enter())
        }
        "focused" -> LaunchedEffect(Unit) {
            interactionSource.emit(FocusInteraction.Focus())
        }
        // Emitting a bare Press (no Hover Enter) drives the press path alone:
        // the driver's synthetic pointer press carries no hover either, and
        // the interior pixels stay within tolerance of Slint's state layer.
        // The flow replays its latest emission, so the button's collectors
        // receive the press whenever they subscribe during the pump. The
        // registration goes to the frame sink, which runs it right after
        // the first presented frame: the Slint driver dispatches //ACTION=
        // just after its own pre-press baseline frame, and an earlier emit
        // would put pressed ink (and the morph's start) into frame 0 — the
        // composition runs ahead of the pump during setup. Landing after
        // frame 0 also keeps the press off uptime 0, where a ripple's frame
        // callback would abort layoutlib.
        "pressed" -> {
            val press = remember {
                Runnable {
                    interactionSource.tryEmit(PressInteraction.Press(pressOffset))
                }
            }
            DisposableEffect(press) {
                val entry = 0L to press
                emitPress.add(entry)
                onDispose { emitPress.remove(entry) }
            }
        }
    }
    // A scene-level `actions` press+release is a click: emit the press and
    // its release at each action's `at` time (back-to-back at frame 0 when
    // untimed — the Slint driver dispatches them at the same mock-clock
    // beat). A `press` only applies to the widget it hit-tests inside —
    // multi-widget scenes must not all light up; the release fires wherever
    // the press landed.
    val pressAction = scene.actions.firstOrNull { it.kind == "press" }
    val releaseAction = scene.actions.firstOrNull { it.kind == "release" }
    if (pressAction != null && widget.state != "pressed") {
        var emitted: PressInteraction.Press? = null
        val press = Runnable {
            val b = tracer.elementBounds[elementId]
            val hits = b == null ||
                (pressAction.x * density >= b.left && pressAction.x * density <= b.right &&
                    pressAction.y * density >= b.top && pressAction.y * density <= b.bottom)
            if (hits) {
                val p = PressInteraction.Press(pressOffset)
                emitted = p
                interactionSource.tryEmit(p)
            }
        }
        DisposableEffect(press) {
            val entry = pressAction.at to press
            emitPress.add(entry)
            onDispose { emitPress.remove(entry) }
        }
        if (releaseAction != null) {
            val release = Runnable {
                emitted?.let { interactionSource.tryEmit(PressInteraction.Release(it)) }
                emitted = null
            }
            DisposableEffect(release) {
                val entry = releaseAction.at to release
                emitPress.add(entry)
                onDispose { emitPress.remove(entry) }
            }
        }
    }
}

/** A press + release pair in the scene's `actions` means a user click —
 * on a checkable widget it toggles `checked`. */
private fun sceneActionsClick(scene: Scene): Boolean =
    scene.actions.any { it.kind == "press" } && scene.actions.any { it.kind == "release" }

/** Loads `icons/<name>.svg` (the file the Slint `Icons.<name>` image
 * renders) as an [ImageVector] so both sides rasterize identical path
 * data. The icon is tinted by `Icon`/`colorize` on top, so a black fill
 * is fine. */
private fun sceneIcon(name: String): androidx.compose.ui.graphics.vector.ImageVector {
    val svg = Scene::class.java.classLoader!!
        .getResourceAsStream("icons/$name.svg")!!
        .bufferedReader().readText()
    val vb = Regex("""viewBox="([\d.\- ]+)"""").find(svg)!!.groupValues[1]
        .trim().split(" ").map { it.toFloat() }
    // `Icon` renders an ImageVector at its intrinsic `defaultWidth/Height`
    // when no explicit size is given, so an icon authored on a non-24
    // viewBox (e.g. the 960-unit Material Symbols grid) must still declare
    // a 24dp size — the viewport maps the path data at any grid size.
    val vpw = vb[2]
    val vph = vb[3]
    val builder = androidx.compose.ui.graphics.vector.ImageVector.Builder(
        name = name,
        defaultWidth = if (vpw >= vph) 24.dp else (24f * vpw / vph).dp,
        defaultHeight = if (vph >= vpw) 24.dp else (24f * vph / vpw).dp,
        viewportWidth = vpw,
        viewportHeight = vph,
    )
    // viewBox carries a nonzero min-x/min-y (e.g. "0 -960 960 960") while
    // the vector viewport always starts at 0 — wrap the paths in a group
    // that shifts them back into view.
    if (vb[0] != 0f || vb[1] != 0f) {
        builder.addGroup(translationX = -vb[0], translationY = -vb[1])
    }
    Regex("""<path[^>]*d="([^"]+)"""").findAll(svg).forEach { m ->
        builder.addPath(
            androidx.compose.ui.graphics.vector.addPathNodes(m.groupValues[1]),
            fill = androidx.compose.ui.graphics.SolidColor(Color.Black),
        )
    }
    if (vb[0] != 0f || vb[1] != 0f) {
        builder.clearGroup()
    }
    return builder.build()
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun StateButton(
    widget: Widget,
    scene: Scene,
    tracer: Tracer,
    textId: String,
    elementId: String,
    density: Float,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
) {
    val interactionSource = remember { ReplayableInteractionSource() }
    emitStateInteractions(widget, scene, tracer, elementId, interactionSource, emitPress, Offset(40f * density, 20f * density), density)

    val h = buttonHeight(widget)
    // The real checked state on a toggle widget; a press+release action
    // sequence flips it from the post-frame-0 hook — the same click the
    // Slint driver's pointer events deliver. A held press (`state =
    // "pressed"`) or a static `checked` flag is not a click.
    var checked by remember { mutableStateOf(widget.checked) }
    val clickToggles = widget.checkable && sceneActionsClick(scene)
    DisposableEffect(Unit) {
        val flip = Runnable { checked = !checked }
        // A click completes on release — `checked` flips at its `at` time.
        val at = scene.actions.firstOrNull { it.kind == "release" }?.at ?: 0L
        val entry = at to flip
        if (clickToggles) {
            emitPress.add(entry)
        }
        onDispose { emitPress.remove(entry) }
    }

    val shapes = buttonShapesFor(widget, h)
    // alpha18's `ToggleButton` has no size parameter (it predates
    // `ToggleButtonSize`) — `shapesFor(h)` still buckets the morph shapes
    // per height, and `Modifier.height(h)` overrides the container's
    // `defaultMinSize(MinHeight)` exactly. The label style is overridden on
    // the `Text` below since alpha18's composable hard-codes `labelLarge`.
    val toggleShapes = ToggleButtonDefaults.shapesFor(h).let {
        if (widget.corner == "square") it.copy(shape = squareShapeFor(widget)) else it
    }
    // `Modifier.height` is exact (not `heightIn`): an extra-small 32dp
    // button is below the composable's internal `defaultMinSize(40dp)` —
    // incoming fixed constraints clamp it correctly.
    val modifier = Modifier.offset(widget.x.dp, widget.y.dp)
        .height(h)
        // A scene may pin the drawn width so the comparator's corner band
        // isn't loosened by text-metric width drift between the engines.
        .then(if (widget.width > 0f) Modifier.width(widget.width.dp) else Modifier)
        .track(tracer, elementId)
    val iconVector = widget.icon?.let { sceneIcon(it) }
    // Plain buttons use `labelLarge` at every height (alpha18 hard-codes it
    // inside `ProvideContentColorTextStyle`); toggles take
    // `textStyleFor(height)` upstream — the alpha18 composable can't, so the
    // style is pinned on the `Text` itself.
    val labelStyle = if (widget.checkable) {
        ButtonDefaults.textStyleFor(h)
    } else {
        androidx.compose.material3.MaterialTheme.typography.labelLarge
    }
    val content: @Composable androidx.compose.foundation.layout.RowScope.() -> Unit = {
        if (iconVector != null) {
            Icon(
                iconVector,
                contentDescription = null,
                modifier = Modifier.size(ButtonDefaults.iconSizeFor(h)),
            )
            Spacer(Modifier.width(ButtonDefaults.iconSpacingFor(h)))
        }
        Text(
            widget.text ?: "",
            style = labelStyle,
            modifier = Modifier.trackText(tracer, textId, density),
            onTextLayout = recordTextLayout(
                tracer,
                textId,
                LocalDensity.current,
                androidx.compose.ui.platform.LocalFontFamilyResolver.current,
                labelStyle.fontFamily,
            ),
        )
    }

    // The corner-radius trace: `AnimatedShapeState` inside the composables
    // is internal, so the probe animates the radius with the spec the
    // MotionScheme hands it (and this side's frames verify the real shape
    // pixels land on the same curve).
    val pressed by interactionSource.collectIsPressedAsState()
    val motionScheme = androidx.compose.material3.MaterialTheme.motionScheme
    val spec = if (widget.checkable) {
        // ToggleButton.kt: `MotionSchemeKeyTokens.FastSpatial` for the morph.
        motionScheme.fastSpatialSpec<Float>()
    } else {
        // Button/IconButton: `MotionSchemeKeyTokens.DefaultEffects`.
        motionScheme.defaultEffectsSpec<Float>()
    }
    val activeShapes = if (widget.checkable) {
        Triple(toggleShapes.shape, toggleShapes.pressedShape, toggleShapes.checkedShape)
    } else {
        Triple(shapes.shape, shapes.pressedShape, shapes.pressedShape)
    }
    val radius by animateFloatAsState(
        targetValue = radiusTarget(
            h.value, activeShapes.first, activeShapes.second, activeShapes.third,
            pressed, widget.checkable && checked, density,
        ),
        animationSpec = spec,
        label = "container_radius",
    )
    tracer.propGetters["container_radius"] = { radius.toDouble() }

    val pressInk = pressInkMarker(widget, emitPress)

    when {
        widget.checkable -> when (widget.kind) {
            "filled-button" -> ToggleButton(
                checked = checked,
                onCheckedChange = {},
                modifier = modifier,
                enabled = widget.enabled,
                shapes = toggleShapes,
                contentPadding = ButtonDefaults.contentPaddingFor(h, hasStartIcon = iconVector != null),
                interactionSource = interactionSource,
                content = content,
            )
            "tonal-button" -> TonalToggleButton(
                checked = checked,
                onCheckedChange = {},
                modifier = modifier,
                enabled = widget.enabled,
                shapes = toggleShapes,
                contentPadding = ButtonDefaults.contentPaddingFor(h, hasStartIcon = iconVector != null),
                interactionSource = interactionSource,
                content = content,
            )
            "elevated-button" -> ElevatedToggleButton(
                checked = checked,
                onCheckedChange = {},
                modifier = modifier,
                enabled = widget.enabled,
                shapes = toggleShapes,
                contentPadding = ButtonDefaults.contentPaddingFor(h, hasStartIcon = iconVector != null),
                interactionSource = interactionSource,
                content = content,
            )
            "outlined-button" -> OutlinedToggleButton(
                checked = checked,
                onCheckedChange = {},
                modifier = modifier,
                enabled = widget.enabled,
                shapes = toggleShapes,
                contentPadding = ButtonDefaults.contentPaddingFor(h, hasStartIcon = iconVector != null),
                interactionSource = interactionSource,
                content = content,
            )
            else -> error("no toggle variant for ${widget.kind}")
        }
        else -> when (widget.kind) {
            "filled-button" -> Button(
                onClick = {},
                enabled = widget.enabled,
                modifier = modifier,
                shapes = shapes,
                contentPadding = ButtonDefaults.contentPaddingFor(h, hasStartIcon = iconVector != null),
                interactionSource = interactionSource,
                content = content,
            )
            "tonal-button" -> FilledTonalButton(
                onClick = {},
                enabled = widget.enabled,
                modifier = modifier,
                shapes = shapes,
                contentPadding = ButtonDefaults.contentPaddingFor(h, hasStartIcon = iconVector != null),
                interactionSource = interactionSource,
                content = content,
            )
            "elevated-button" -> ElevatedButton(
                onClick = {},
                enabled = widget.enabled,
                modifier = modifier,
                shapes = shapes,
                contentPadding = ButtonDefaults.contentPaddingFor(h, hasStartIcon = iconVector != null),
                interactionSource = interactionSource,
                content = content,
            )
            "outlined-button" -> OutlinedButton(
                onClick = {},
                enabled = widget.enabled,
                modifier = modifier,
                shapes = shapes,
                contentPadding = ButtonDefaults.contentPaddingFor(h, hasStartIcon = iconVector != null),
                interactionSource = interactionSource,
                content = content,
            )
            "text-button" -> TextButton(
                onClick = {},
                enabled = widget.enabled,
                modifier = modifier,
                shapes = shapes,
                contentPadding = ButtonDefaults.contentPaddingFor(h, hasStartIcon = iconVector != null),
                interactionSource = interactionSource,
                content = content,
            )
            else -> error("unknown text button kind ${widget.kind}")
        }
    }

    if (pressInk.value) {
        PressInkOverlay(
            widget,
            tracer,
            elementId,
            density,
            // The ink is clipped to the live morph radius — the same spring
            // curve the widget's internal AnimatedShapeState follows — so it
            // stays inside the drawn arc mid-morph like upstream's own clip.
            RoundedCornerShape(radius.dp),
            buttonInkColor(widget, checked),
        )
    }
}

/** `ButtonShapes` for a text-button widget: `shapesFor` buckets the pressed
 * shape per height; `square` swaps the resting stadium for the size's
 * `ContainerShapeSquare` token. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun buttonShapesFor(widget: Widget, h: Dp): androidx.compose.material3.ButtonShapes {
    val base = ButtonDefaults.shapesFor(h)
    return if (widget.corner == "square") {
        base.copy(shape = squareShapeFor(widget))
    } else {
        base
    }
}

private fun RoundedCornerShapeOrRect(radius: Dp): Shape =
    if (radius <= 0.dp) RectangleShape else androidx.compose.foundation.shape.RoundedCornerShape(radius)

/** The M3 elevation level → dp mapping `ElevationTokens` generates for
 * Slint (`level1 = 1dp` … `level5 = 12dp`). */
fun Widget.elevationDp(): Dp =
    when (level) {
        1 -> 1.dp
        2 -> 3.dp
        3 -> 6.dp
        4 -> 8.dp
        5 -> 12.dp
        else -> 0.dp
    }

/** Same level→dp table for `sheet_elevation`; -1 = the pinned default
 * (`BottomSheetDefaults.Elevation` for scaffold shadows). */
@OptIn(ExperimentalMaterial3Api::class)
fun Widget.sheetElevationDp(): Dp =
    when (sheetElevation) {
        -1 -> BottomSheetDefaults.Elevation
        1 -> 1.dp
        2 -> 3.dp
        3 -> 6.dp
        4 -> 8.dp
        5 -> 12.dp
        else -> 0.dp
    }

/** The `MaterialShapes` polygon a scene's `surface` widget names — the same
 * kebab name `MaterialShapes.<kebab>` exposes on the Slint side. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun materialShape(name: String): Shape =
    (
        when (name) {
            "circle" -> androidx.compose.material3.MaterialShapes.Circle
            "square" -> androidx.compose.material3.MaterialShapes.Square
            "slanted" -> androidx.compose.material3.MaterialShapes.Slanted
            "arch" -> androidx.compose.material3.MaterialShapes.Arch
            "fan" -> androidx.compose.material3.MaterialShapes.Fan
            "arrow" -> androidx.compose.material3.MaterialShapes.Arrow
            "semi-circle" -> androidx.compose.material3.MaterialShapes.SemiCircle
            "oval" -> androidx.compose.material3.MaterialShapes.Oval
            "pill" -> androidx.compose.material3.MaterialShapes.Pill
            "triangle" -> androidx.compose.material3.MaterialShapes.Triangle
            "diamond" -> androidx.compose.material3.MaterialShapes.Diamond
            "clam-shell" -> androidx.compose.material3.MaterialShapes.ClamShell
            "pentagon" -> androidx.compose.material3.MaterialShapes.Pentagon
            "gem" -> androidx.compose.material3.MaterialShapes.Gem
            "sunny" -> androidx.compose.material3.MaterialShapes.Sunny
            "very-sunny" -> androidx.compose.material3.MaterialShapes.VerySunny
            "cookie-4-sided" -> androidx.compose.material3.MaterialShapes.Cookie4Sided
            "cookie-6-sided" -> androidx.compose.material3.MaterialShapes.Cookie6Sided
            "cookie-7-sided" -> androidx.compose.material3.MaterialShapes.Cookie7Sided
            "cookie-9-sided" -> androidx.compose.material3.MaterialShapes.Cookie9Sided
            "cookie-12-sided" -> androidx.compose.material3.MaterialShapes.Cookie12Sided
            "ghostish" -> androidx.compose.material3.MaterialShapes.Ghostish
            "clover-4-leaf" -> androidx.compose.material3.MaterialShapes.Clover4Leaf
            "clover-8-leaf" -> androidx.compose.material3.MaterialShapes.Clover8Leaf
            "burst" -> androidx.compose.material3.MaterialShapes.Burst
            "soft-burst" -> androidx.compose.material3.MaterialShapes.SoftBurst
            "boom" -> androidx.compose.material3.MaterialShapes.Boom
            "soft-boom" -> androidx.compose.material3.MaterialShapes.SoftBoom
            "flower" -> androidx.compose.material3.MaterialShapes.Flower
            "puffy" -> androidx.compose.material3.MaterialShapes.Puffy
            "puffy-diamond" -> androidx.compose.material3.MaterialShapes.PuffyDiamond
            "pixel-circle" -> androidx.compose.material3.MaterialShapes.PixelCircle
            "pixel-triangle" -> androidx.compose.material3.MaterialShapes.PixelTriangle
            "bun" -> androidx.compose.material3.MaterialShapes.Bun
            "heart" -> androidx.compose.material3.MaterialShapes.Heart
            else -> error("scene catalog has no MaterialShapes member for $name")
        }
    ).toShape()

@Composable
fun Widget.outline(): Shape =
    if (shape == "rect") RoundedCornerShapeOrRect(radius.dp) else materialShape(shape)

@Composable
private fun schemeColor(role: String): Color =
    androidx.compose.material3.MaterialTheme.colorScheme.let { scheme ->
        when (role.kebabToCamel()) {
            "primary" -> scheme.primary
            "onPrimary" -> scheme.onPrimary
            "primaryContainer" -> scheme.primaryContainer
            "onPrimaryContainer" -> scheme.onPrimaryContainer
            "inversePrimary" -> scheme.inversePrimary
            "secondary" -> scheme.secondary
            "onSecondary" -> scheme.onSecondary
            "secondaryContainer" -> scheme.secondaryContainer
            "onSecondaryContainer" -> scheme.onSecondaryContainer
            "tertiary" -> scheme.tertiary
            "onTertiary" -> scheme.onTertiary
            "tertiaryContainer" -> scheme.tertiaryContainer
            "onTertiaryContainer" -> scheme.onTertiaryContainer
            "background" -> scheme.background
            "onBackground" -> scheme.onBackground
            "surface" -> scheme.surface
            "onSurface" -> scheme.onSurface
            "surfaceVariant" -> scheme.surfaceVariant
            "surfaceTint" -> scheme.surfaceTint
            "inverseSurface" -> scheme.inverseSurface
            "inverseOnSurface" -> scheme.inverseOnSurface
            "error" -> scheme.error
            "onError" -> scheme.onError
            "errorContainer" -> scheme.errorContainer
            "onErrorContainer" -> scheme.onErrorContainer
            "outline" -> scheme.outline
            "outlineVariant" -> scheme.outlineVariant
            "scrim" -> scheme.scrim
            "surfaceBright" -> scheme.surfaceBright
            "surfaceDim" -> scheme.surfaceDim
            "surfaceContainer" -> scheme.surfaceContainer
            "surfaceContainerHigh" -> scheme.surfaceContainerHigh
            "surfaceContainerHighest" -> scheme.surfaceContainerHighest
            "surfaceContainerLow" -> scheme.surfaceContainerLow
            "surfaceContainerLowest" -> scheme.surfaceContainerLowest
            "primaryFixed" -> scheme.primaryFixed
            "primaryFixedDim" -> scheme.primaryFixedDim
            "onPrimaryFixed" -> scheme.onPrimaryFixed
            "onPrimaryFixedVariant" -> scheme.onPrimaryFixedVariant
            "secondaryFixed" -> scheme.secondaryFixed
            "secondaryFixedDim" -> scheme.secondaryFixedDim
            "onSecondaryFixed" -> scheme.onSecondaryFixed
            "onSecondaryFixedVariant" -> scheme.onSecondaryFixedVariant
            "tertiaryFixed" -> scheme.tertiaryFixed
            "tertiaryFixedDim" -> scheme.tertiaryFixedDim
            "onTertiaryFixed" -> scheme.onTertiaryFixed
            "onTertiaryFixedVariant" -> scheme.onTertiaryFixedVariant
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

/** `IconButtonDefaults.*ContainerSize(widthOption)` for the widget's size
 * bucket and width option. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun iconContainerSize(widget: Widget): androidx.compose.ui.unit.DpSize = when (widget.size) {
    "xs" -> IconButtonDefaults.extraSmallContainerSize(iconWidthOption(widget))
    "s" -> IconButtonDefaults.smallContainerSize(iconWidthOption(widget))
    "m" -> IconButtonDefaults.mediumContainerSize(iconWidthOption(widget))
    "l" -> IconButtonDefaults.largeContainerSize(iconWidthOption(widget))
    "xl" -> IconButtonDefaults.extraLargeContainerSize(iconWidthOption(widget))
    else -> error("unknown button size ${widget.size}")
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
private fun iconWidthOption(widget: Widget) = when (widget.widthOption) {
    "narrow" -> IconButtonDefaults.IconButtonWidthOption.Narrow
    "uniform" -> IconButtonDefaults.IconButtonWidthOption.Uniform
    "wide" -> IconButtonDefaults.IconButtonWidthOption.Wide
    else -> error("unknown width option ${widget.widthOption}")
}

/** `IconButtonDefaults.*IconSize` for the size bucket. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
private fun iconSize(widget: Widget) = when (widget.size) {
    "xs" -> IconButtonDefaults.extraSmallIconSize
    "s" -> IconButtonDefaults.smallIconSize
    "m" -> IconButtonDefaults.mediumIconSize
    "l" -> IconButtonDefaults.largeIconSize
    "xl" -> IconButtonDefaults.extraLargeIconSize
    else -> error("unknown button size ${widget.size}")
}

/** Resting shape of an icon button: the size's `ContainerShapeRound` or
 * `ContainerShapeSquare` token (`IconButtonDefaults.*{Round,Square}Shape`). */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun iconRestingShape(widget: Widget): Shape {
    val d = IconButtonDefaults
    return when {
        widget.corner == "square" -> when (widget.size) {
            "xs" -> d.extraSmallSquareShape
            "s" -> d.smallSquareShape
            "m" -> d.mediumSquareShape
            "l" -> d.largeSquareShape
            "xl" -> d.extraLargeSquareShape
            else -> error("unknown button size ${widget.size}")
        }
        else -> when (widget.size) {
            "xs" -> d.extraSmallRoundShape
            "s" -> d.smallRoundShape
            "m" -> d.mediumRoundShape
            "l" -> d.largeRoundShape
            "xl" -> d.extraLargeRoundShape
            else -> error("unknown button size ${widget.size}")
        }
    }
}

/** Pressed shape per size (`IconButtonDefaults.*PressedShape`). */
@Composable
private fun iconPressedShape(widget: Widget): Shape = when (widget.size) {
    "xs" -> IconButtonDefaults.extraSmallPressedShape
    "s" -> IconButtonDefaults.smallPressedShape
    "m" -> IconButtonDefaults.mediumPressedShape
    "l" -> IconButtonDefaults.largePressedShape
    "xl" -> IconButtonDefaults.extraLargePressedShape
    else -> error("unknown button size ${widget.size}")
}

/** Checked shape of an icon toggle: the size's `SelectedContainerShapeRound`
 * or `SelectedContainerShapeSquare`. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun iconCheckedShape(widget: Widget): Shape {
    val d = IconButtonDefaults
    return when {
        widget.corner == "square" -> when (widget.size) {
            "xs" -> d.extraSmallSelectedSquareShape
            "s" -> d.smallSelectedSquareShape
            "m" -> d.mediumSelectedSquareShape
            "l" -> d.largeSelectedSquareShape
            "xl" -> d.extraLargeSelectedSquareShape
            else -> error("unknown button size ${widget.size}")
        }
        else -> when (widget.size) {
            "xs" -> d.extraSmallSelectedRoundShape
            "s" -> d.smallSelectedRoundShape
            "m" -> d.mediumSelectedRoundShape
            "l" -> d.largeSelectedRoundShape
            "xl" -> d.extraLargeSelectedRoundShape
            else -> error("unknown button size ${widget.size}")
        }
    }
}

/** An icon button in any style/state — the `standard`/`filled`/`tonal`/
 * `outlined` container, its `narrow`/`uniform`/`wide` container size, the
 * round/square shape, and the `*ToggleButton` checked variants. Standard
 * and outlined pick up `LocalContentColor` upstream — the scene uses the
 * `*Vibrant*` color variants so the reference matches the Slint side's
 * token-pinned colors. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun StateIconButton(
    widget: Widget,
    scene: Scene,
    tracer: Tracer,
    elementId: String,
    density: Float,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
) {
    val interactionSource = remember { ReplayableInteractionSource() }
    val containerSize = iconContainerSize(widget)
    emitStateInteractions(
        widget,
        scene,
        tracer,
        elementId,
        interactionSource,
        emitPress,
        Offset(containerSize.width.value * density / 2f, containerSize.height.value * density / 2f),
        density,
    )

    var checked by remember { mutableStateOf(widget.checked) }
    val clickToggles = widget.checkable && sceneActionsClick(scene)
    DisposableEffect(Unit) {
        val flip = Runnable { checked = !checked }
        // A click completes on release — `checked` flips at its `at` time.
        val at = scene.actions.firstOrNull { it.kind == "release" }?.at ?: 0L
        val entry = at to flip
        if (clickToggles) {
            emitPress.add(entry)
        }
        onDispose { emitPress.remove(entry) }
    }

    val shapes = IconButtonDefaults.shapes(
        shape = iconRestingShape(widget),
        pressedShape = iconPressedShape(widget),
    )
    val toggleShapes = IconButtonDefaults.toggleableShapes(
        shape = iconRestingShape(widget),
        pressedShape = iconPressedShape(widget),
        checkedShape = iconCheckedShape(widget),
    )
    val modifier = Modifier.offset(widget.x.dp, widget.y.dp)
        .size(containerSize)
        .track(tracer, elementId)
    val iconVector = sceneIcon(widget.icon ?: "check")
    val content: @Composable () -> Unit = {
        Icon(iconVector, contentDescription = null, modifier = Modifier.size(iconSize(widget)))
    }

    // Same corner-radius probe as the text buttons — the composable's
    // AnimatedShapeState is internal; the trace re-derives it.
    val pressed by interactionSource.collectIsPressedAsState()
    val spec = androidx.compose.material3.MaterialTheme.motionScheme.defaultEffectsSpec<Float>()
    val radius by animateFloatAsState(
        targetValue = radiusTarget(
            containerSize.height.value,
            iconRestingShape(widget), iconPressedShape(widget), iconCheckedShape(widget),
            pressed, widget.checkable && checked, density,
        ),
        animationSpec = spec,
        label = "container_radius",
    )
    tracer.propGetters["container_radius"] = { radius.toDouble() }

    val pressInk = pressInkMarker(widget, emitPress)

    when {
        widget.checkable -> when (widget.kind) {
            "icon-button" -> IconToggleButton(
                checked = checked,
                onCheckedChange = {},
                modifier = modifier,
                enabled = widget.enabled,
                shapes = toggleShapes,
                colors = IconButtonDefaults.iconToggleButtonVibrantColors(),
                interactionSource = interactionSource,
                content = content,
            )
            "filled-icon-button" -> FilledIconToggleButton(
                checked = checked,
                onCheckedChange = {},
                modifier = modifier,
                enabled = widget.enabled,
                shapes = toggleShapes,
                interactionSource = interactionSource,
                content = content,
            )
            "tonal-icon-button" -> FilledTonalIconToggleButton(
                checked = checked,
                onCheckedChange = {},
                modifier = modifier,
                enabled = widget.enabled,
                shapes = toggleShapes,
                interactionSource = interactionSource,
                content = content,
            )
            "outlined-icon-button" -> OutlinedIconToggleButton(
                checked = checked,
                onCheckedChange = {},
                modifier = modifier,
                enabled = widget.enabled,
                shapes = toggleShapes,
                colors = IconButtonDefaults.outlinedIconToggleButtonVibrantColors(),
                // The default border factory keys on `LocalContentColor`;
                // the vibrant factory uses the spec's `OutlineColor` token.
                border = IconButtonDefaults.outlinedIconToggleButtonVibrantBorder(widget.enabled, checked),
                interactionSource = interactionSource,
                content = content,
            )
            else -> error("no icon toggle variant for ${widget.kind}")
        }
        else -> when (widget.kind) {
            "icon-button" -> IconButton(
                onClick = {},
                shapes = shapes,
                modifier = modifier,
                enabled = widget.enabled,
                colors = IconButtonDefaults.iconButtonVibrantColors(),
                interactionSource = interactionSource,
                content = content,
            )
            "filled-icon-button" -> FilledIconButton(
                onClick = {},
                shapes = shapes,
                modifier = modifier,
                enabled = widget.enabled,
                interactionSource = interactionSource,
                content = content,
            )
            "tonal-icon-button" -> FilledTonalIconButton(
                onClick = {},
                shapes = shapes,
                modifier = modifier,
                enabled = widget.enabled,
                interactionSource = interactionSource,
                content = content,
            )
            "outlined-icon-button" -> OutlinedIconButton(
                onClick = {},
                shapes = shapes,
                modifier = modifier,
                enabled = widget.enabled,
                colors = IconButtonDefaults.outlinedIconButtonVibrantColors(),
                // Same `LocalContentColor`-vs-token split as the toggle.
                border = IconButtonDefaults.outlinedIconButtonVibrantBorder(widget.enabled),
                interactionSource = interactionSource,
                content = content,
            )
            else -> error("unknown icon button kind ${widget.kind}")
        }
    }

    if (pressInk.value) {
        PressInkOverlay(
            widget,
            tracer,
            elementId,
            density,
            // Live morph radius — see the button overlay above.
            RoundedCornerShape(radius.dp),
            iconInkColor(widget, checked),
        )
    }
}

/** One corner of a token `RoundedCornerShape`, resolved in dp — the split
 * halves' morphing corners are `CornerSize`s; `Size(h, h)` reproduces
 * `PercentCornerSize`'s `minDimension` since the container height is the
 * smaller dimension in these scenes. `corner` picks the side: the leading
 * morphs its end corners (`{ it.topEnd }`), the trailing its start
 * corners (`{ it.topStart }`). */
private fun splitCornerDp(
    shape: Shape,
    corner: (androidx.compose.foundation.shape.RoundedCornerShape) -> androidx.compose.foundation.shape.CornerSize,
    h: androidx.compose.ui.unit.Dp,
    density: Float,
): Float {
    require(shape is androidx.compose.foundation.shape.RoundedCornerShape) {
        "token shapes are RoundedCornerShape, got $shape"
    }
    val boxPx = h.value * density
    return corner(shape).toPx(
        androidx.compose.ui.geometry.Size(boxPx, boxPx),
        androidx.compose.ui.unit.Density(density),
    ) / density
}

/** A split button — `SplitButtonLayout`'s leading action plus the
 * toggleable trailing menu button. `side` picks which half the authored
 * `state` and the scripted pointer gesture target; a click on the trailing
 * flips `checked` (the menu-open state upstream's `DropdownMenu(expanded)`
 * mirrors). `leadingButtonShapesFor(h)`/`trailingButtonShapesFor(h)` carry
 * the outer/inner/inner-pressed corner tokens; the trailing's checked
 * shape is `CircleShape` — the h/2 inner target below. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun StateSplitButton(
    widget: Widget,
    scene: Scene,
    tracer: Tracer,
    textId: String,
    elementId: String,
    density: Float,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
) {
    val h = buttonHeight(widget)
    val d = SplitButtonDefaults
    // Sub-element ids for the pointer hit-test — internal only: they are
    // never named by `trace_elements`, so the frame dump stays one element.
    val leadingId = "${elementId}_leading"
    val trailingId = "${elementId}_trailing"
    val leadingSource = remember { ReplayableInteractionSource() }
    val trailingSource = remember { ReplayableInteractionSource() }
    val sideTrailing = widget.side == "trailing"
    emitStateInteractions(
        widget,
        scene,
        tracer,
        if (sideTrailing) trailingId else leadingId,
        if (sideTrailing) trailingSource else leadingSource,
        emitPress,
        Offset(h.value * density / 2f, h.value * density / 2f),
        density,
    )

    var checked by remember { mutableStateOf(widget.checked) }
    val clickToggles = widget.checkable && sceneActionsClick(scene)
    DisposableEffect(Unit) {
        // A click completes on release; it only toggles when the gesture
        // hit the trailing half — a leading press fires `leading_clicked`
        // upstream instead.
        val flip = Runnable {
            val b = tracer.elementBounds[trailingId]
            val p = scene.actions.first { it.kind == "press" }
            if (b == null ||
                (p.x * density >= b.left && p.x * density <= b.right &&
                    p.y * density >= b.top && p.y * density <= b.bottom)
            ) {
                checked = !checked
            }
        }
        val at = scene.actions.firstOrNull { it.kind == "release" }?.at ?: 0L
        val entry = at to flip
        if (clickToggles) {
            emitPress.add(entry)
        }
        onDispose { emitPress.remove(entry) }
    }

    // alpha18's `SplitButtonShapes` fields are nullable (`hasPressedShape`
    // tracks it); the `*ButtonShapesFor` factories always populate them.
    val leadingShapes = d.leadingButtonShapesFor(h)
    val leadingRest = checkNotNull(leadingShapes.shape)
    val leadingPress = checkNotNull(leadingShapes.pressedShape)
    val trailingShapes = d.trailingButtonShapesFor(h)
    val trailingRest = checkNotNull(trailingShapes.shape)
    val trailingPress = checkNotNull(trailingShapes.pressedShape)

    // Inner-corner morph probes — the same `DefaultEffects` spec
    // (`MotionSchemeKeyTokens.DefaultEffects`) the halves' internal
    // AnimatedShapeState animates through (SplitButton.kt `shapeByInteraction`).
    val leadingPressed by leadingSource.collectIsPressedAsState()
    val trailingPressed by trailingSource.collectIsPressedAsState()
    val spec = androidx.compose.material3.MaterialTheme.motionScheme.defaultEffectsSpec<Float>()
    val leadingInner by animateFloatAsState(
        targetValue = splitCornerDp(
            if (leadingPressed) leadingPress else leadingRest,
            { it.topEnd },
            h,
            density,
        ),
        animationSpec = spec,
        label = "leading_inner_radius",
    )
    val trailingInner by animateFloatAsState(
        targetValue = when {
            trailingPressed ->
                splitCornerDp(trailingPress, { it.topStart }, h, density)
            checked -> h.value / 2f
            else -> splitCornerDp(trailingRest, { it.topStart }, h, density)
        },
        animationSpec = spec,
        label = "trailing_inner_radius",
    )
    tracer.propGetters["leading_inner_radius"] = { leadingInner.toDouble() }
    tracer.propGetters["trailing_inner_radius"] = { trailingInner.toDouble() }

    // The samples' `animateFloatAsState(if (checked) 180f else 0f)` —
    // `animateFloatAsState`'s default spec is `spring(dampingRatio = 1,
    // stiffness = 1500)`, the same `spring(1, 1500)` the Slint side puts on
    // `animated_icon_rotation`.
    val iconRotation by animateFloatAsState(
        targetValue = if (checked) 180f else 0f,
        label = "trailing_icon_rotation",
    )
    tracer.propGetters["trailing_icon_rotation"] = { iconRotation.toDouble() }

    // Both halves' content sits under `ProvideContentColorTextStyle
    // (labelLarge)` upstream — no per-size text style on a split button.
    val labelStyle = androidx.compose.material3.MaterialTheme.typography.labelLarge
    val leadingContent: @Composable androidx.compose.foundation.layout.RowScope.() -> Unit = {
        widget.icon?.let { icon ->
            Icon(
                sceneIcon(icon),
                contentDescription = null,
                modifier = Modifier.size(d.leadingButtonIconSizeFor(h)),
            )
            Spacer(Modifier.width(ButtonDefaults.iconSpacingFor(h)))
        }
        // No `Text` for an empty label: Slint drops the element entirely, so
        // tracking one here would only report `no Slint Text element`.
        if (!widget.text.isNullOrEmpty()) {
            Text(
                widget.text,
                style = labelStyle,
                modifier = Modifier.trackText(tracer, textId, density),
                onTextLayout = recordTextLayout(
                    tracer,
                    textId,
                    LocalDensity.current,
                    androidx.compose.ui.platform.LocalFontFamilyResolver.current,
                    labelStyle.fontFamily,
                ),
            )
        }
    }
    val trailingContent: @Composable androidx.compose.foundation.layout.RowScope.() -> Unit = {
        Icon(
            sceneIcon(widget.trailingIcon ?: "keyboard_arrow_down"),
            contentDescription = null,
            modifier = Modifier
                .size(d.trailingButtonIconSizeFor(h))
                .graphicsLayer { rotationZ = iconRotation },
        )
    }
    val pressInk = pressInkMarker(widget, emitPress)

    // The halves' own floor is `SmallContainerHeight` (40dp); the samples
    // pin the bucket's height per half — `Modifier.heightIn(size)` — so
    // this side does the same (SplitButtonSamples.kt).
    val leadingModifier = Modifier.heightIn(min = h).track(tracer, leadingId)
    val trailingModifier = Modifier.heightIn(min = h).track(tracer, trailingId)
    val modifier = Modifier.offset(widget.x.dp, widget.y.dp)
        .then(if (widget.width > 0f) Modifier.width(widget.width.dp) else Modifier)
        .track(tracer, elementId)
    val leading: @Composable () -> Unit = {
        when (widget.kind) {
            "filled-split-button" -> d.LeadingButton(
                onClick = {},
                modifier = leadingModifier,
                enabled = widget.enabled,
                shapes = leadingShapes,
                contentPadding = d.leadingButtonContentPaddingFor(h),
                interactionSource = leadingSource,
                content = leadingContent,
            )
            "tonal-split-button" -> d.TonalLeadingButton(
                onClick = {},
                modifier = leadingModifier,
                enabled = widget.enabled,
                shapes = leadingShapes,
                contentPadding = d.leadingButtonContentPaddingFor(h),
                interactionSource = leadingSource,
                content = leadingContent,
            )
            "elevated-split-button" -> d.ElevatedLeadingButton(
                onClick = {},
                modifier = leadingModifier,
                enabled = widget.enabled,
                shapes = leadingShapes,
                contentPadding = d.leadingButtonContentPaddingFor(h),
                interactionSource = leadingSource,
                content = leadingContent,
            )
            "outlined-split-button" -> d.OutlinedLeadingButton(
                onClick = {},
                modifier = leadingModifier,
                enabled = widget.enabled,
                shapes = leadingShapes,
                contentPadding = d.leadingButtonContentPaddingFor(h),
                interactionSource = leadingSource,
                content = leadingContent,
            )
            else -> error("unknown split button kind ${widget.kind}")
        }
    }
    val trailing: @Composable () -> Unit = {
        when (widget.kind) {
            "filled-split-button" -> d.TrailingButton(
                checked = checked,
                onCheckedChange = {},
                modifier = trailingModifier,
                enabled = widget.enabled,
                shapes = trailingShapes,
                contentPadding = d.trailingButtonContentPaddingFor(h),
                interactionSource = trailingSource,
                content = trailingContent,
            )
            "tonal-split-button" -> d.TonalTrailingButton(
                checked = checked,
                onCheckedChange = {},
                modifier = trailingModifier,
                enabled = widget.enabled,
                shapes = trailingShapes,
                contentPadding = d.trailingButtonContentPaddingFor(h),
                interactionSource = trailingSource,
                content = trailingContent,
            )
            "elevated-split-button" -> d.ElevatedTrailingButton(
                checked = checked,
                onCheckedChange = {},
                modifier = trailingModifier,
                enabled = widget.enabled,
                shapes = trailingShapes,
                contentPadding = d.trailingButtonContentPaddingFor(h),
                interactionSource = trailingSource,
                content = trailingContent,
            )
            "outlined-split-button" -> d.OutlinedTrailingButton(
                checked = checked,
                onCheckedChange = {},
                modifier = trailingModifier,
                enabled = widget.enabled,
                shapes = trailingShapes,
                contentPadding = d.trailingButtonContentPaddingFor(h),
                interactionSource = trailingSource,
                content = trailingContent,
            )
            else -> error("unknown split button kind ${widget.kind}")
        }
    }
    SplitButtonLayout(
        leadingButton = leading,
        trailingButton = trailing,
        modifier = modifier,
    )

    if (pressInk.value) {
        val inkColor = when (widget.kind) {
            "filled-split-button" -> ButtonDefaults.buttonColors().contentColor
            "tonal-split-button" -> ButtonDefaults.filledTonalButtonColors().contentColor
            "elevated-split-button" -> ButtonDefaults.elevatedButtonColors().contentColor
            "outlined-split-button" -> ButtonDefaults.outlinedButtonColors().contentColor
            else -> error("unknown split button kind ${widget.kind}")
        }
        PressInkOverlay(
            widget,
            tracer,
            if (sideTrailing) trailingId else leadingId,
            density,
            // Stadium bound on the held half — `mask_inner` covers the
            // mid-morph corner band in timed scenes.
            RoundedCornerShape(h / 2),
            inkColor,
        )
    }
}

/** `StateTokens.PressedStateLayerOpacity` — internal upstream, pinned here
 * by reference: 0.1 of the content color at the held-press settle. */
private const val PRESSED_STATE_LAYER_ALPHA = 0.1f

/** The content color a widget's ripple tints its container with — the same
 * color object family the dispatch below picks for the composable. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun buttonInkColor(widget: Widget, checked: Boolean): Color {
    val b = ButtonDefaults
    val t = ToggleButtonDefaults
    return when {
        widget.checkable -> when (widget.kind) {
            "filled-button" -> t.toggleButtonColors()
            "tonal-button" -> t.tonalToggleButtonColors()
            "elevated-button" -> t.elevatedToggleButtonColors()
            "outlined-button" -> t.outlinedToggleButtonColors()
            else -> error("no toggle variant for ${widget.kind}")
        }.let { if (checked) it.checkedContentColor else it.contentColor }
        else -> when (widget.kind) {
            "filled-button" -> b.buttonColors().contentColor
            "tonal-button" -> b.filledTonalButtonColors().contentColor
            "elevated-button" -> b.elevatedButtonColors().contentColor
            "outlined-button" -> b.outlinedButtonColors().contentColor
            "text-button" -> b.textButtonColors().contentColor
            else -> error("unknown text button kind ${widget.kind}")
        }
    }
}

/** Same for the icon-button variants: vibrant where the spec keys on
 * `LocalContentColor`, the defaults elsewhere. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun iconInkColor(widget: Widget, checked: Boolean): Color {
    val d = IconButtonDefaults
    return when {
        widget.checkable -> when (widget.kind) {
            "icon-button" -> d.iconToggleButtonVibrantColors()
            "filled-icon-button" -> d.filledIconToggleButtonColors()
            "tonal-icon-button" -> d.filledTonalIconToggleButtonColors()
            "outlined-icon-button" -> d.outlinedIconToggleButtonVibrantColors()
            else -> error("no icon toggle variant for ${widget.kind}")
        }.let { if (checked) it.checkedContentColor else it.contentColor }
        else -> when (widget.kind) {
            "icon-button" -> d.iconButtonVibrantColors().contentColor
            "filled-icon-button" -> d.filledIconButtonColors().contentColor
            "tonal-icon-button" -> d.filledTonalIconButtonColors().contentColor
            "outlined-icon-button" -> d.outlinedIconButtonVibrantColors().contentColor
            else -> error("unknown icon button kind ${widget.kind}")
        }
    }
}

/** Flips once the frame-sink press lands — the same slot the bare `Press`
 * emit uses, after the baseline frame 0. On a device the held ripple
 * settles to its ink; layoutlib never advances that animator, so the
 * [PressInkOverlay] reproduces the settled visual instead. */
@Composable
private fun pressInkMarker(
    widget: Widget,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
): androidx.compose.runtime.State<Boolean> {
    val show = remember { mutableStateOf(false) }
    if (widget.state != "pressed") {
        return show
    }
    val mark = remember { Runnable { show.value = true } }
    DisposableEffect(mark) {
        val entry = 0L to mark
        emitPress.add(entry)
        onDispose { emitPress.remove(entry) }
    }
    return show
}

/** The settled held-press ink: the content color at
 * [PRESSED_STATE_LAYER_ALPHA] inside the live (pressed) shape, drawn over
 * the widget's tracked bounds. Painting over the content is exact rather
 * than approximate: the ink color is the content color, so glyph pixels
 * keep their tone. */
@Composable
private fun PressInkOverlay(
    widget: Widget,
    tracer: Tracer,
    elementId: String,
    density: Float,
    shape: Shape,
    color: Color,
) {
    val b = tracer.elementBounds[elementId] ?: return
    Box(
        Modifier.offset((b.left / density).dp, (b.top / density).dp)
            .size((b.width / density).dp, (b.height / density).dp)
            .clip(shape)
            .background(color.copy(alpha = PRESSED_STATE_LAYER_ALPHA)),
    )
}


/** App-bar family widgets (`top-app-bar` variants, `bottom-app-bar`,
 * `search-bar`, `app-bar-with-search`): geometry plus the props the Slint
 * parity case sets. Elements are named `appbar{n}` in scene order. Window
 * insets are zeroed — the Slint side has no inset concept. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun StateAppBar(widget: Widget, tracer: Tracer, tag: String) {
    val modifier =
        Modifier.offset(widget.x.dp, widget.y.dp)
            .width(widget.width.dp)
            .track(tracer, tag)
    val navIcon: @Composable (() -> Unit)? = widget.navIcon?.let { icon ->
        { IconButton(onClick = {}) { Icon(sceneIcon(icon), contentDescription = null) } }
    }
    val actions: @Composable androidx.compose.foundation.layout.RowScope.() -> Unit = {
        widget.icons.forEach { icon ->
            IconButton(onClick = {}) { Icon(sceneIcon(icon), contentDescription = null) }
        }
    }
    when (widget.kind) {
        "top-app-bar" -> {
            val twoRow = widget.variant == "medium-flexible" ||
                widget.variant == "large-flexible" ||
                widget.variant == "two-rows"
            val state = rememberTopAppBarState(
                initialHeightOffsetLimit = if (twoRow) -Float.MAX_VALUE else 0f,
                initialHeightOffset = widget.heightOffset,
                initialContentOffset = widget.contentOffset,
            )
            val scrollBehavior =
                if (twoRow) TopAppBarDefaults.exitUntilCollapsedScrollBehavior(state)
                else TopAppBarDefaults.pinnedScrollBehavior(state)
            val title: @Composable () -> Unit = { Text(widget.text ?: "") }
            val subtitle: (@Composable () -> Unit)? = widget.subtitle?.let { { Text(it) } }
            val insets = WindowInsets(0, 0, 0, 0)
            when (widget.variant) {
                // The no-subtitle overloads keep the single-line paddings —
                // the subtitle overload changes the title slot's layout.
                "small" -> if (subtitle == null) TopAppBar(
                    title, modifier,
                    navigationIcon = navIcon ?: {},
                    actions = actions,
                    windowInsets = insets,
                    scrollBehavior = scrollBehavior,
                ) else TopAppBar(
                    title, subtitle, modifier,
                    navigationIcon = navIcon ?: {},
                    actions = actions,
                    windowInsets = insets,
                    scrollBehavior = scrollBehavior,
                )
                "center" -> CenterAlignedTopAppBar(
                    title, modifier,
                    navigationIcon = navIcon ?: {},
                    actions = actions,
                    windowInsets = insets,
                    scrollBehavior = scrollBehavior,
                )
                "medium" -> MediumTopAppBar(
                    title, modifier,
                    navigationIcon = navIcon ?: {},
                    actions = actions,
                    windowInsets = insets,
                    scrollBehavior = scrollBehavior,
                )
                "medium-flexible" -> MediumFlexibleTopAppBar(
                    title, modifier,
                    subtitle = subtitle,
                    navigationIcon = navIcon ?: {},
                    actions = actions,
                    windowInsets = insets,
                    scrollBehavior = scrollBehavior,
                )
                "large" -> LargeTopAppBar(
                    title, modifier,
                    navigationIcon = navIcon ?: {},
                    actions = actions,
                    windowInsets = insets,
                    scrollBehavior = scrollBehavior,
                )
                "large-flexible" -> LargeFlexibleTopAppBar(
                    title, modifier,
                    subtitle = subtitle,
                    navigationIcon = navIcon ?: {},
                    actions = actions,
                    windowInsets = insets,
                    scrollBehavior = scrollBehavior,
                )
                "two-rows" -> TwoRowsTopAppBar(
                    { title() }, modifier,
                    subtitle = subtitle?.let { sub -> { sub() } },
                    navigationIcon = navIcon ?: {},
                    actions = actions,
                    windowInsets = insets,
                    scrollBehavior = scrollBehavior,
                )
                else -> error("unknown top-app-bar variant ${widget.variant}")
            }
        }
        "bottom-app-bar" -> {
            val state = rememberBottomAppBarState(
                initialHeightOffsetLimit = -Float.MAX_VALUE,
                initialHeightOffset = widget.heightOffset,
                initialContentOffset = widget.contentOffset,
            )
            val fab: (@Composable () -> Unit)? = widget.navIcon?.let { icon ->
                { androidx.compose.material3.FloatingActionButton(
                    onClick = {},
                    modifier = Modifier.track(tracer, "fab"),
                ) {
                    Icon(sceneIcon(icon), contentDescription = null) } }
            }
            BottomAppBar(
                actions = actions,
                modifier = modifier,
                floatingActionButton = fab,
                windowInsets = WindowInsets(0, 0, 0, 0),
                scrollBehavior = BottomAppBarDefaults.exitAlwaysScrollBehavior(state),
            )
        }
        "search-bar" -> {
            val field: @Composable () -> Unit = {
                SearchBarDefaults.InputField(
                    query = widget.text ?: "",
                    onQueryChange = {},
                    onSearch = {},
                    expanded = false,
                    onExpandedChange = {},
                    placeholder = widget.placeholder?.let { { Text(it) } },
                    leadingIcon = navIcon,
                    trailingIcon = widget.icons.firstOrNull()?.let { icon ->
                        { Icon(sceneIcon(icon), contentDescription = null) }
                    },
                )
            }
            SearchBar(
                state = rememberSearchBarState(),
                inputField = field,
                modifier = modifier,
            )
        }
        "app-bar-with-search" -> {
            val field: @Composable () -> Unit = {
                SearchBarDefaults.InputField(
                    query = widget.text ?: "",
                    onQueryChange = {},
                    onSearch = {},
                    expanded = false,
                    onExpandedChange = {},
                    placeholder = widget.placeholder?.let { { Text(it) } },
                )
            }
            AppBarWithSearch(
                state = rememberSearchBarState(),
                inputField = field,
                modifier = modifier,
                navigationIcon = navIcon,
                actions = actions,
                windowInsets = WindowInsets(0, 0, 0, 0),
            )
        }
        else -> error("unknown app-bar kind ${widget.kind}")
    }
}

// ── Bottom sheets ──────────────────────────────────────────────────────────
// Mirrors BottomSheet.kt, BottomSheetScaffold.kt, ModalBottomSheet.kt,
// SheetDefaults.kt, DragHandle.kt and AnchoredDraggable.kt at the pinned
// androidx commit (23327507f7fc).

@OptIn(ExperimentalMaterial3Api::class)
private fun sheetValue(name: String): SheetValue = when (name) {
    "hidden" -> SheetValue.Hidden
    "partially-expanded" -> SheetValue.PartiallyExpanded
    "expanded" -> SheetValue.Expanded
    else -> error("unknown sheet value $name")
}

/** `SheetValue` → the numeric trace proxy both engines emit:
 * hidden = 0, partially-expanded = 1, expanded = 2. */
@OptIn(ExperimentalMaterial3Api::class)
private fun sheetValueNum(value: SheetValue): Double = when (value) {
    SheetValue.Hidden -> 0.0
    SheetValue.PartiallyExpanded -> 1.0
    SheetValue.Expanded -> 2.0
}

/** A pointer `actions` entry inside the widget's box means the scene wants a
 * real drag replayed — routed to [ParitySheet], which drives the public
 * `AnchoredDraggableState` the composables use internally. Pointer-free
 * scenes get the real composables ([SheetWidget]) so the rendered pixels
 * come from the same code path an app would use. */
@OptIn(ExperimentalMaterial3Api::class)
private fun sheetNeedsGestureReplay(widget: Widget, scene: Scene): Boolean =
    scene.actions.any { a ->
        (a.kind == "press" || a.kind == "move" || a.kind == "release") &&
            a.x >= widget.x && a.x <= widget.x + widget.width &&
            a.y >= widget.y && a.y <= widget.y + widget.height
    }

/** Registers the programmatic sheet ops (`show`/`hide`/`expand`/
 * `partial-expand`) the scene's `actions` declare as post-frame-0 runnables —
 * the same public `SheetState` calls the pinned composables' callbacks make
 * (a scrim tap is `sheetState.hide()`, ModalBottomSheet.kt's
 * `animateToDismiss`). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun registerSheetOps(
    scene: Scene,
    show: suspend (String) -> Unit,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
) {
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    val runnables = scene.actions.mapNotNull { a ->
        when (a.kind) {
            "show", "hide", "expand", "partial-expand" -> {
                val kind = a.kind
                a.at to Runnable { scope.launch { show(kind) } }
            }
            else -> null
        }
    }
    DisposableEffect(Unit) {
        runnables.forEach { emitPress.add(it) }
        onDispose { runnables.forEach { emitPress.remove(it) } }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
private suspend fun sheetOp(state: SheetState, kind: String) = when (kind) {
    "show" -> state.show()
    "hide" -> state.hide()
    "expand" -> state.expand()
    "partial-expand" -> state.partialExpand()
    else -> error("unknown sheet op $kind")
}

@OptIn(ExperimentalMaterial3Api::class)
private suspend fun sheetOp(state: AnchoredDraggableState<SheetValue>, kind: String,
    showSpec: androidx.compose.animation.core.AnimationSpec<Float>,
    anchoredSpec: androidx.compose.animation.core.AnimationSpec<Float>,
    hideSpec: androidx.compose.animation.core.AnimationSpec<Float>,
) {
    // SheetDefaults.kt at the pin: `show`/`expand` run on showMotionSpec
    // (defaultSpatial), `partialExpand`/`hide` on hideMotionSpec (fastEffects).
    when (kind) {
        "show" -> state.animateTo(
            if (state.anchors.hasPositionFor(SheetValue.PartiallyExpanded))
                SheetValue.PartiallyExpanded else SheetValue.Expanded,
            showSpec,
        )
        "hide" -> state.animateTo(SheetValue.Hidden, hideSpec)
        "expand" -> state.animateTo(SheetValue.Expanded, showSpec)
        "partial-expand" -> state.animateTo(SheetValue.PartiallyExpanded, hideSpec)
        else -> error("unknown sheet op $kind")
    }
}

@OptIn(ExperimentalMaterial3Api::class)
private fun registerSheetTraceProps(tracer: Tracer, offsetPx: () -> Float,
    currentValue: () -> SheetValue, targetValue: () -> SheetValue,
    isVisible: () -> Boolean, dragging: () -> Boolean,
    hasExpanded: () -> Boolean, hasPartial: () -> Boolean,
    sheetHeightDp: Float, density: Float,
    surfaceTag: String? = null, uiDensity: androidx.compose.ui.unit.Density? = null,
) {
    // propGetters are looked up per name the scene lists in `trace_props`;
    // unregistered names are simply skipped in the recorded frame.
    tracer.propGetters["sheet-offset"] = {
        val v = offsetPx()
        if (surfaceTag != null && uiDensity != null) {
            // `sheet-pill{i}` tracks the live offset, not the modifier's
            // static bounds: the drag-handle pill rides the surface, so its
            // bounds are offset + DragHandleVerticalPadding (22 dp) under
            // the surface's top edge, centered, 32x4 dp.
            val s = tracer.elementBounds[surfaceTag]
            if (s != null && !v.isNaN()) {
                val w32 = with(uiDensity) { 32.dp.toPx() }
                val h4 = with(uiDensity) { 4.dp.toPx() }
                val v22 = with(uiDensity) { 22.dp.toPx() }
                val pill = "sheet-pill" + surfaceTag.filter(Char::isDigit)
                val pb = Rect(
                    s.left + (s.width - w32) / 2f,
                    v + v22,
                    s.left + (s.width - w32) / 2f + w32,
                    v + v22 + h4,
                )
                tracer.elementBounds[pill] = pb
                tracer.backfill(pill, pb)
                // `sheet-surface{i}` is the moving surface itself: same x/w
                // as the tracked modifier, top at the live offset, height
                // `sheetHeightDp` — the metrics the Slint proxy exposes.
                val tag = "sheet-surface" + surfaceTag.filter(Char::isDigit)
                val maxW = with(uiDensity) { 640.dp.toPx() }
                val sw = minOf(s.width, maxW)
                val sb = Rect(
                    s.left + (s.width - sw) / 2f,
                    v,
                    s.left + (s.width - sw) / 2f + sw,
                    v + with(uiDensity) { sheetHeightDp.dp.toPx() },
                )
                tracer.elementBounds[tag] = sb
                tracer.backfill(tag, sb)
                // `sheet-content{i}` is the surface minus its 48 dp handle
                // strip — the handle/content edge is a painted boundary that
                // `mask_decor` can reach only through a traced element.
                val handlePx = with(uiDensity) { 48.dp.toPx() }
                val ctag = "sheet-content" + surfaceTag.filter(Char::isDigit)
                val cb = Rect(sb.left, v + handlePx, sb.right, sb.bottom)
                tracer.elementBounds[ctag] = cb
                tracer.backfill(ctag, cb)
            }
        }
        if (v.isNaN()) Double.NaN else (v / density).toDouble()
    }
    tracer.propGetters["sheet-height"] = { sheetHeightDp.toDouble() }
    tracer.propGetters["current-value"] = { sheetValueNum(currentValue()) }
    tracer.propGetters["target-value"] = { sheetValueNum(targetValue()) }
    tracer.propGetters["is-visible"] = { isVisible() }
    tracer.propGetters["dragging"] = { dragging() }
    tracer.propGetters["has-expanded-state"] = { hasExpanded() }
    tracer.propGetters["has-partially-expanded-state"] = { hasPartial() }
}

/** DragHandle.kt's VerticalDragHandle — the real composable, so its pill
 * geometry comes from `VerticalDragHandleDefaults` at the pin. `isPressed`
 * is a pressable-internal `remember` var and can't be faked through the
 * interaction source, but at this token pin `pressed_*` and `dragged_*`
 * produce the identical 12x52 / onSurface / cornerMedium pill, so a
 * `DragInteraction.Start` covers both scene states. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun StateVerticalDragHandle(
    widget: Widget,
    scene: Scene,
    tracer: Tracer,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
    tag: String,
) {
    val source = remember { ReplayableInteractionSource() }
    // `pressable` reports through the hoisted source, so the trace props
    // read the composable's own interaction bookkeeping.
    val isPressed by source.collectIsPressedAsState()
    val isDragged by source.collectIsDraggedAsState()
    tracer.propGetters["pressed"] = { isPressed }
    tracer.propGetters["dragged"] = { isDragged }
    tracer.propGetters["active"] = { isPressed || isDragged }
    CompositionLocalProvider(LocalMinimumInteractiveComponentSize provides 48.dp) {
        VerticalDragHandle(
            modifier = Modifier.offset(widget.x.dp, widget.y.dp)
                .track(tracer, tag),
            interactionSource = source,
        )
    }
    if (widget.state == "pressed" || widget.state == "dragged") {
        val emit = remember {
            Runnable { source.tryEmit(androidx.compose.foundation.interaction.DragInteraction.Start()) }
        }
        DisposableEffect(emit) {
            val entry = 0L to emit
            emitPress.add(entry)
            onDispose { emitPress.remove(entry) }
        }
    }
}

/** The real composables — used by every scene without pointer actions on the
 * sheet, including programmatic-op motion scenes (`show`/`hide`/`expand`/
 * `partial-expand` runnables drive the public `SheetState`). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SheetWidget(
    widget: Widget,
    scene: Scene,
    tracer: Tracer,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
    tag: String,
) {
    val density = androidx.compose.ui.platform.LocalDensity.current.density
    val uiDensity = androidx.compose.ui.platform.LocalDensity.current
    when (widget.kind) {
        "bottom-sheet" -> {
            // The published alpha18 jar predates the pin's
            // `rememberBottomSheetState`: the public surface to a non-default
            // initial value is the companion `Saver`, whose `restore` builds
            // a `SheetState` with the given flags and initial value.
            val state = remember {
                SheetState.Saver(
                    skipPartiallyExpanded = widget.skipPartial,
                    positionalThreshold = { with(uiDensity) { 56.dp.toPx() } },
                    velocityThreshold = { with(uiDensity) { 125.dp.toPx() } },
                    confirmValueChange = { true },
                    skipHiddenState = false,
                ).restore(sheetValue(widget.initial))
                    ?: error("SheetState.Saver.restore returned null")
            }
            registerSheetOps(scene, { sheetOp(state, it) }, emitPress)
            registerSheetTraceProps(
                tracer,
                offsetPx = { if (state.hasExpandedState) state.requireOffset() else Float.NaN },
                currentValue = { state.currentValue },
                targetValue = { state.targetValue },
                isVisible = { state.isVisible },
                dragging = { false },
                hasExpanded = { state.hasExpandedState },
                hasPartial = { state.hasPartiallyExpandedState },
                sheetHeightDp = widget.sheetHeight + 48f,
                density = density,
                surfaceTag = tag,
                uiDensity = uiDensity,
            )
            Box(Modifier.offset(widget.x.dp, widget.y.dp).size(widget.width.dp, widget.height.dp)) {
                BottomSheet(
                    state = state,
                    gesturesEnabled = widget.gestures,
                    contentWindowInsets = { WindowInsets(0, 0, 0, 0) },
                    modifier = Modifier.trackSheet(tracer, tag, uiDensity),
                ) {
                    Box(
                        Modifier.fillMaxWidth()
                            .height(widget.sheetHeight.dp)
                            .background(schemeColor(widget.contentColor)),
                    )
                }
            }
        }
        "bottom-sheet-scaffold" -> {
            // `rememberStandardBottomSheetState` — the alpha18 public API —
            // maps straight to the pin's scaffold state flags.
            val state = rememberStandardBottomSheetState(
                initialValue = sheetValue(
                    if (widget.initial == "hidden" && widget.skipHidden)
                        "partially-expanded" else widget.initial
                ),
                confirmValueChange = { true },
                skipHiddenState = widget.skipHidden,
            )
            val scaffoldState = rememberBottomSheetScaffoldState(bottomSheetState = state)
            registerSheetOps(scene, { sheetOp(state, it) }, emitPress)
            registerSheetTraceProps(
                tracer,
                offsetPx = { if (state.hasExpandedState) state.requireOffset() else Float.NaN },
                currentValue = { state.currentValue },
                targetValue = { state.targetValue },
                isVisible = { state.isVisible },
                dragging = { false },
                hasExpanded = { state.hasExpandedState },
                hasPartial = { state.hasPartiallyExpandedState },
                sheetHeightDp = widget.sheetHeight + 48f,
                density = density,
                surfaceTag = tag,
                uiDensity = uiDensity,
            )
            Box(Modifier.offset(widget.x.dp, widget.y.dp).size(widget.width.dp, widget.height.dp)) {
                BottomSheetScaffold(
                    scaffoldState = scaffoldState,
                    sheetPeekHeight = widget.peekHeight.dp,
                    // Platform shadows deadlock layoutlib — scenes pass
                    // sheet_elevation 0; shadow parity lives in the elevation
                    // scenes.
                    sheetShadowElevation = widget.sheetElevationDp(),
                    sheetSwipeEnabled = widget.gestures,
                    snackbarHost = {},
                    sheetContent = {
                        Box(
                            Modifier.fillMaxWidth()
                                .height(widget.sheetHeight.dp)
                                .background(schemeColor(widget.contentColor)),
                        )
                    },
                    modifier = Modifier.trackSheet(tracer, tag, uiDensity),
                ) {
                    Box(
                        Modifier.fillMaxSize()
                            .background(schemeColor(widget.bodyColor)),
                    )
                }
            }
        }
        "modal-bottom-sheet" -> {
            // ModalBottomSheet.kt:139-160 — the sheet lives in a Dialog the
            // scene can't host, so the mirror inlines its Box{Scrim +
            // BottomSheet} at the widget's bounds; everything below the pin's
            // `content` lambda is the real composable.
            val state = rememberModalBottomSheetState(
                skipPartiallyExpanded = widget.skipPartial,
            )
            registerSheetOps(scene, { sheetOp(state, it) }, emitPress)
            registerSheetTraceProps(
                tracer,
                offsetPx = { if (state.hasExpandedState) state.requireOffset() else Float.NaN },
                currentValue = { state.currentValue },
                targetValue = { state.targetValue },
                isVisible = { state.isVisible },
                dragging = { false },
                hasExpanded = { state.hasExpandedState },
                hasPartial = { state.hasPartiallyExpandedState },
                sheetHeightDp = widget.sheetHeight + 48f,
                density = density,
                surfaceTag = tag,
                uiDensity = uiDensity,
            )
            LaunchedEffect(state.hasExpandedState) {
                if (state.hasExpandedState) state.show()
            }
            Box(Modifier.offset(widget.x.dp, widget.y.dp).size(widget.width.dp, widget.height.dp)) {
                val scrimAlpha by animateFloatAsState(
                    targetValue = if (state.targetValue != SheetValue.Hidden) 1f else 0f,
                    animationSpec =
                        androidx.compose.material3.MaterialTheme.motionScheme
                            .defaultEffectsSpec(),
                    label = "ScrimAlphaAnimation",
                )
                Box(
                    Modifier.fillMaxSize()
                        .background(
                            BottomSheetDefaults.ScrimColor.copy(alpha = scrimAlpha)
                        ),
                )
                Box(Modifier.align(Alignment.TopCenter)) {
                    BottomSheet(
                        state = state,
                        gesturesEnabled = widget.gestures,
                        contentWindowInsets = { WindowInsets(0, 0, 0, 0) },
                        modifier = Modifier.trackSheet(tracer, tag, uiDensity),
                    ) {
                        Box(
                            Modifier.fillMaxWidth()
                                .height(widget.sheetHeight.dp)
                                .background(schemeColor(widget.contentColor)),
                        )
                    }
                }
            }
        }
    }
}

/** The gesture-replay sheet: pointer `actions` are replayed on the public
 * `AnchoredDraggableState` — the same class BottomSheetImpl uses internally
 * — so the drag and fling paths execute the pin's real clamping, target
 * selection, and dampening. Anchor tables and the fling prelude are copied
 * from the pinned sources. `isVisible`/dismiss is rendered but not
 * propagated (there is no Dialog to close). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ParitySheet(
    widget: Widget,
    scene: Scene,
    tracer: Tracer,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
    tag: String,
) {
    val density = androidx.compose.ui.platform.LocalDensity.current.density
    val uiDensity = androidx.compose.ui.platform.LocalDensity.current
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    val isScaffold = widget.kind == "bottom-sheet-scaffold"
    val isModal = widget.kind == "modal-bottom-sheet"
    val layoutH = widget.height
    val sheetH = widget.sheetHeight + 48f
    val peekPx = widget.peekHeight

    val initial = sheetValue(
        if (isScaffold && widget.initial == "hidden") "partially-expanded" else widget.initial
    )
    val state = remember { AnchoredDraggableState<SheetValue>(initial) }
    val anchors = remember {
        androidx.compose.foundation.gestures.DraggableAnchors {
            // `DraggableAnchors` positions are pixels; the widget numbers are dp.
            if (isScaffold) {
                // BottomSheetScaffold.kt:297-343 — declaration order P, H, E.
                val partialAvailable =
                    !widget.skipPartial && peekPx > 0f && peekPx != sheetH
                val hiddenAvailable = sheetH == 0f || peekPx == 0f || !widget.skipHidden
                if (partialAvailable) {
                    SheetValue.PartiallyExpanded at (layoutH - peekPx) * density
                }
                if (hiddenAvailable) {
                    SheetValue.Hidden at layoutH * density
                }
                if (sheetH > 0f) {
                    SheetValue.Expanded at (layoutH - sheetH) * density
                }
            } else {
                // BottomSheet.kt:299-308 — declaration order H, P, E; the
                // deterministic `isPartiallyExpandedAnchorAvailable` and
                // `calculatePartiallyExpandedOffset` (fullHeight - min(half,
                // sheetHeight)).
                SheetValue.Hidden at layoutH * density
                if (!widget.skipPartial) {
                    SheetValue.PartiallyExpanded at
                        (layoutH - kotlin.math.min(layoutH / 2f, sheetH)) * density
                }
                if (sheetH != 0f) {
                    SheetValue.Expanded at kotlin.math.max(0f, layoutH - sheetH) * density
                }
            }
        }
    }
    // The pin's resolve table on first anchor update
    // (BottomSheetScaffold.kt:345-360 / BottomSheet.kt:311-326); old anchors
    // are empty so `shouldPromoteToExpanded` can never fire.
    val resolvedTarget = when (initial) {
        SheetValue.Hidden ->
            if (isScaffold && !anchors.hasPositionFor(SheetValue.Hidden)) initial
            else SheetValue.Hidden
        SheetValue.PartiallyExpanded -> when {
            anchors.hasPositionFor(SheetValue.PartiallyExpanded) -> SheetValue.PartiallyExpanded
            anchors.hasPositionFor(SheetValue.Expanded) -> SheetValue.Expanded
            anchors.hasPositionFor(SheetValue.Hidden) -> SheetValue.Hidden
            else -> initial
        }
        SheetValue.Expanded ->
            if (anchors.hasPositionFor(SheetValue.Expanded)) SheetValue.Expanded
            else SheetValue.Hidden
    }
    androidx.compose.runtime.SideEffect { state.updateAnchors(anchors, resolvedTarget) }

    val viewConfiguration = androidx.compose.ui.platform.LocalViewConfiguration.current
    val spatialFlingSpec =
        androidx.compose.material3.MaterialTheme.motionScheme.defaultSpatialSpec<Float>()
    val showSpec = spatialFlingSpec
    val hideSpec =
        androidx.compose.material3.MaterialTheme.motionScheme.fastEffectsSpec<Float>()
    if (isModal) {
        // ModalBottomSheet.kt — `LaunchedEffect { if (hasExpandedState)
        // { show() } }`: the modal opens itself once anchors exist.
        androidx.compose.runtime.LaunchedEffect(state.anchors.hasPositionFor(SheetValue.Expanded)) {
            if (state.anchors.hasPositionFor(SheetValue.Expanded)) {
                sheetOp(state, "show", showSpec, showSpec, hideSpec)
            }
        }
    }
    val anchoredDraggableFlingBehavior =
        androidx.compose.foundation.gestures.AnchoredDraggableDefaults.flingBehavior(
            state = state,
            // BottomSheetDefaults.PositionalThreshold — internal, 56.dp.
            positionalThreshold = { _ ->
                with(uiDensity) { 56.dp.toPx() }
            },
            animationSpec = spatialFlingSpec,
        )
    val flingBehavior = androidx.compose.runtime.remember(
        anchoredDraggableFlingBehavior, viewConfiguration, density,
    ) {
        if (isScaffold) {
            anchoredDraggableFlingBehavior
        } else {
            // BottomSheet.kt:219-256 — `modalBottomSheetFlingBehavior`, the
            // same BoundaryDampeningZone prelude the docked and modal sheets
            // share.
            object : androidx.compose.foundation.gestures.FlingBehavior {
                override suspend fun androidx.compose.foundation.gestures.ScrollScope.performFling(
                    initialVelocity: Float,
                ): Float {
                    val maxSystemVelocity = viewConfiguration.maximumFlingVelocity
                    var safeVelocity =
                        initialVelocity.coerceIn(-maxSystemVelocity, maxSystemVelocity)
                    if (safeVelocity > 0f && anchors.hasPositionFor(SheetValue.Hidden)) {
                        val hiddenAnchor = anchors.positionOf(SheetValue.Hidden)
                        val currentOffset = state.requireOffset()
                        val distanceToFloor = kotlin.math.max(0f, hiddenAnchor - currentOffset)
                        // BottomSheetDefaults.BoundaryDampeningZone — 125.dp.
                        val dampeningZone =
                            with(uiDensity) {
                                125.dp.toPx()
                            }
                        if (distanceToFloor < dampeningZone) {
                            val factor = distanceToFloor / dampeningZone
                            safeVelocity *= (factor * factor)
                            // BottomSheetDefaults.VelocityThreshold — 125.dp.
                            val velocityThresholdPx =
                                with(uiDensity) {
                                    125.dp.toPx()
                                }
                            if (initialVelocity >= velocityThresholdPx) {
                                safeVelocity = kotlin.math.max(safeVelocity, velocityThresholdPx)
                            }
                        }
                    }
                    return with(anchoredDraggableFlingBehavior) { performFling(safeVelocity) }
                }
            }
        }
    }

    // Replay state — the press baseline for `move` deltas and the dragging
    // trace flag.
    var pressed by remember { androidx.compose.runtime.mutableStateOf(false) }
    var lastY = 0f
    val sheetOpsEnabled = widget.gestures

    val runnables = scene.actions.mapNotNull { a ->
        val insideSheet = a.y >= widget.y &&
            a.y <= widget.y + widget.height && a.x >= widget.x && a.x <= widget.x + widget.width
        when {
            a.kind == "press" && insideSheet -> a.at to Runnable {
                if (isModal && a.y * density < state.offset) {
                    // Above the sheet's top edge: the pin's Scrim consumes the
                    // tap and calls animateToDismiss → sheetState.hide().
                    // UNDISPATCHED starts the op in this frame — a plain
                    // launch lands a variable 1-3 frames later and makes the
                    // spring's start nondeterministic vs the Slint timeline.
                    scope.launch(start = kotlinx.coroutines.CoroutineStart.UNDISPATCHED) {
                        sheetOp(state, "hide", showSpec, showSpec, hideSpec)
                    }
                } else if (sheetOpsEnabled) {
                    pressed = true
                    lastY = a.y * density
                }
            }
            // A move past the sheet's edge is still part of the gesture —
            // only the initiating press is bounds-checked.
            a.kind == "move" -> a.at to Runnable {
                if (pressed && sheetOpsEnabled) {
                    val y = a.y * density
                    // The real drag pipes each delta through dragTo —
                    // dispatchRawDelta is the same call (AnchoredDraggable.kt
                    // :1248).
                    state.dispatchRawDelta(y - lastY)
                    lastY = y
                }
            }
            a.kind == "release" -> a.at to Runnable {
                if (pressed && sheetOpsEnabled) {
                    pressed = false
                    val v = a.velocity * density
                    scope.launch(start = kotlinx.coroutines.CoroutineStart.UNDISPATCHED) {
                        state.anchoredDrag {
                            val scrollScope = object :
                                androidx.compose.foundation.gestures.ScrollScope {
                                override fun scrollBy(pixels: Float): Float =
                                    state.dispatchRawDelta(pixels)
                            }
                            with(flingBehavior) { scrollScope.performFling(v) }
                        }
                    }
                }
            }
            a.kind == "show" || a.kind == "hide" || a.kind == "expand" ||
                a.kind == "partial-expand" -> a.at to Runnable {
                val kind = a.kind
                scope.launch(start = kotlinx.coroutines.CoroutineStart.UNDISPATCHED) {
                    sheetOp(state, kind, showSpec, showSpec, hideSpec)
                }
            }
            else -> null
        }
    }
    DisposableEffect(Unit) {
        runnables.forEach { emitPress.add(it) }
        onDispose { runnables.forEach { emitPress.remove(it) } }
    }

    registerSheetTraceProps(
        tracer,
        offsetPx = { state.offset },
        currentValue = { state.currentValue },
        targetValue = { state.targetValue },
        isVisible = { state.currentValue != SheetValue.Hidden },
        dragging = { pressed },
        hasExpanded = { state.anchors.hasPositionFor(SheetValue.Expanded) },
        hasPartial = { state.anchors.hasPositionFor(SheetValue.PartiallyExpanded) },
        sheetHeightDp = sheetH,
        density = density,
        surfaceTag = tag,
        uiDensity = uiDensity,
    )

    Box(Modifier.offset(widget.x.dp, widget.y.dp).size(widget.width.dp, widget.height.dp)) {
        if (isModal) {
            val scrimAlpha by animateFloatAsState(
                targetValue = if (state.targetValue != SheetValue.Hidden) 1f else 0f,
                animationSpec =
                    androidx.compose.material3.MaterialTheme.motionScheme.defaultEffectsSpec(),
                label = "ScrimAlphaAnimation",
            )
            Box(
                Modifier.fillMaxSize()
                    .background(BottomSheetDefaults.ScrimColor.copy(alpha = scrimAlpha)),
            )
        }
        if (isScaffold) {
            Box(Modifier.fillMaxSize().background(schemeColor(widget.bodyColor)))
        }
        // BottomSheet.kt's `verticalScaleUp` (:340-360): past the topmost
        // anchor the surface stretches so its floor stays on the min anchor;
        // the column's content keeps natural height pinned to the top.
        val offsetDp = if (state.offset.isNaN()) layoutH else state.offset / density
        val minAnchorDp = state.anchors.minPosition() / density
        val stretched = sheetH + kotlin.math.max(0f, minAnchorDp - offsetDp)
        Column(
            Modifier.align(Alignment.TopCenter)
                .widthIn(max = BottomSheetDefaults.SheetMaxWidth)
                .fillMaxWidth()
                .offset { IntOffset(0, (offsetDp * density).roundToInt()) }
                .requiredHeight(stretched.dp)
                .clip(BottomSheetDefaults.ExpandedShape)
                .background(BottomSheetDefaults.ContainerColor)
                .track(tracer, tag),
        ) {
            Box(
                Modifier.fillMaxWidth().height(48.dp),
                contentAlignment = Alignment.Center,
            ) {
                BottomSheetDefaults.DragHandle()
            }
            Box(
                Modifier.fillMaxWidth()
                    .height(widget.sheetHeight.dp)
                    .background(schemeColor(widget.contentColor)),
            )
        }
    }
}
