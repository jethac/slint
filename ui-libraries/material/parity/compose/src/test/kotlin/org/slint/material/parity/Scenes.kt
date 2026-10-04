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
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.TweenSpec
import androidx.compose.animation.core.VectorConverter
import androidx.compose.animation.core.updateTransition
import androidx.compose.foundation.interaction.DragInteraction
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.shape.CornerSize
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.ToggleButtonShapes
import androidx.compose.material3.ElevatedButton
import androidx.compose.material3.ElevatedToggleButton
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.DividerDefaults
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
import androidx.compose.material3.AssistChip
import androidx.compose.material3.AssistChipDefaults
import androidx.compose.material3.ElevatedAssistChip
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.ElevatedFilterChip
import androidx.compose.material3.InputChip
import androidx.compose.material3.InputChipDefaults
import androidx.compose.material3.SuggestionChip
import androidx.compose.material3.ElevatedSuggestionChip
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.SegmentedListItem
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.ContainedLoadingIndicator
import androidx.compose.material3.LocalMinimumInteractiveComponentSize
import androidx.compose.material3.LocalRippleThemeConfiguration
import androidx.compose.material3.RippleDefaults
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.FloatingActionButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LargeExtendedFloatingActionButton
import androidx.compose.material3.LargeFloatingActionButton
import androidx.compose.material3.MediumExtendedFloatingActionButton
import androidx.compose.material3.MediumFloatingActionButton
import androidx.compose.material3.SmallExtendedFloatingActionButton
import androidx.compose.material3.SmallFloatingActionButton
import androidx.compose.material3.animateFloatingActionButton
import androidx.compose.material3.SplitButtonDefaults
import androidx.compose.material3.SplitButtonLayout
import androidx.compose.material3.MaterialExpressiveTheme
import androidx.compose.material3.MotionScheme
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedIconButton
import androidx.compose.material3.OutlinedIconToggleButton
import androidx.compose.material3.OutlinedToggleButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.ToggleButton
import androidx.compose.material3.ToggleButtonDefaults
import androidx.compose.material3.Typography
import androidx.compose.material3.VerticalDivider
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.BadgeDefaults
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
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.requiredHeight
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.gestures.AnchoredDraggableDefaults
import androidx.compose.foundation.gestures.AnchoredDraggableState
import androidx.compose.foundation.gestures.DraggableAnchors
import androidx.compose.foundation.gestures.FlingBehavior
import androidx.compose.foundation.gestures.ScrollScope
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
import androidx.compose.runtime.withFrameNanos
import kotlin.math.roundToInt
import kotlinx.coroutines.launch
import androidx.compose.ui.graphics.vector.path
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.State
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
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.Dp
import androidx.compose.foundation.interaction.Interaction
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
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
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
        var items = 0
        var appbars = 0
        var sheets = 0
        var vhandles = 0
        var groups = 0
        var icons = 0
        var dividers = 0
        var badges = 0
        var badgedBoxes = 0
        // `text:{n}` spans every text node in scene order — group items
        // interleave with the standalone widgets' labels. Bases are
        // precomputed per widget so recompositions can't renumber them.
        var texts = 0
        val textBases = scene.widgets.map { w ->
            (texts).also {
                texts += when {
                    w.kind == "connected-button-group" ||
                        w.kind == "vertical-connected-button-group" -> w.items.size
                    w.kind == "connected-button" || w.isButton || w.kind.endsWith("chip") -> 1
                    else -> 0
                }
            }
        }
        scene.widgets.forEachIndexed { widgetIndex, widget ->
            val textBase = textBases[widgetIndex]
            when {
                widget.kind == "connected-button" -> StateConnectedButton(
                    widget,
                    scene,
                    tracer,
                    "text:$textBase",
                    "button${buttons++}",
                    density,
                    emitPress,
                )
                widget.kind == "connected-button-group" ||
                    widget.kind == "vertical-connected-button-group" ->
                    StateConnectedGroup(
                        widget,
                        scene,
                        tracer,
                        "group${groups++}",
                        density,
                        emitPress,
                        textBase,
                    )
                widget.kind.endsWith("chip") -> StateChip(
                    widget,
                    scene,
                    tracer,
                    "text:$textBase",
                    "button${buttons++}",
                    density,
                    emitPress,
                )
                widget.isFab -> StateFab(
                    widget,
                    scene,
                    tracer,
                    "text:${buttons}",
                    "button${buttons++}",
                    density,
                    emitPress,
                )
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
                widget.kind == "switch" -> StateSwitch(
                    widget,
                    scene,
                    tracer,
                    "button${buttons++}",
                    density,
                    emitPress,
                )
                widget.isButton -> StateButton(
                    widget,
                    scene,
                    tracer,
                    "text:$textBase",
                    "button${buttons++}",
                    density,
                    emitPress,
                )
                widget.isListItem -> {
                    // `text:<n>` ids pair with the Slint side's Text
                    // elements in document order — each item emits its
                    // avatar label, overline, headline, supporting, and
                    // trailing text in that order.
                    val nTexts = 1 +
                        listOfNotNull(
                            widget.avatar,
                            widget.overline,
                            widget.supporting,
                            widget.trailingText,
                        ).size
                    StateListItem(
                        widget,
                        scene,
                        tracer,
                        texts,
                        "item${items++}",
                        density,
                        emitPress,
                    )
                    texts += nTexts
                }
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
                widget.kind == "icon" -> {
                    // The bare `Icon` takes its painter's intrinsic size
                    // (or `DefaultIconSizeModifier`'s 24dp when the
                    // painter has none); `width`/`height` apply upstream's
                    // `modifier` size and `color` applies `tint`. The
                    // default tint is `LocalContentColor` — provided here
                    // as `onSurface`, the content color a `Surface` gives
                    // (the same value as the Slint default `on_background`
                    // at the pin).
                    val tag = "icon${icons++}"
                    CompositionLocalProvider(
                        androidx.compose.material3.LocalContentColor provides
                            scheme.onSurface,
                    ) {
                        Icon(
                            sceneIcon(widget.icon ?: "check"),
                            contentDescription = null,
                            modifier = Modifier.offset(widget.x.dp, widget.y.dp)
                                .then(
                                    if (widget.width > 0) {
                                        Modifier.width(widget.width.dp)
                                    } else {
                                        Modifier
                                    },
                                )
                                .then(
                                    if (widget.height > 0) {
                                        Modifier.height(widget.height.dp)
                                    } else {
                                        Modifier
                                    },
                                )
                                .track(tracer, tag),
                            tint = widget.color?.let { schemeColor(it) }
                                ?: androidx.compose.material3.LocalContentColor.current,
                        )
                    }
                }
                widget.kind == "divider" || widget.kind == "horizontal-divider" ||
                    widget.kind == "vertical-divider" -> {
                    // `Divider`/`HorizontalDivider` is `fillMaxWidth().
                    // height(thickness)`; `VerticalDivider` is
                    // `fillMaxHeight().width(thickness)` — the band's long
                    // axis comes from the scene's span, the short axis is
                    // the composable's `thickness` (0 = `Dp.Hairline`).
                    val tag = "divider${dividers++}"
                    val color = widget.color?.let { schemeColor(it) } ?: DividerDefaults.color
                    val thickness = widget.thickness.dp
                    Box(
                        Modifier.offset(widget.x.dp, widget.y.dp)
                            .then(
                                if (widget.kind == "vertical-divider") {
                                    Modifier.height(widget.height.dp)
                                } else {
                                    Modifier.width(widget.width.dp)
                                },
                            ),
                    ) {
                        if (widget.kind == "divider") {
                            @Suppress("DEPRECATION")
                            androidx.compose.material3.Divider(
                                thickness = thickness,
                                color = color,
                            )
                        } else if (widget.kind == "vertical-divider") {
                            VerticalDivider(
                                modifier = Modifier.track(tracer, tag),
                                thickness = thickness,
                                color = color,
                            )
                        } else {
                            HorizontalDivider(
                                modifier = Modifier.track(tracer, tag),
                                thickness = thickness,
                                color = color,
                            )
                        }
                    }
                }
                widget.kind == "badge" -> {
                    // Upstream `Badge`: an empty `text` is the null-content
                    // branch (the `BadgeTokens.Size` dot), any text the
                    // large badge — a null-and-empty content lambda is not
                    // the same thing upstream, so the two calls differ in
                    // kind, not just arguments.
                    val tag = "badge${badges++}"
                    val container = widget.color?.let { schemeColor(it) }
                        ?: BadgeDefaults.containerColor
                    val mods = Modifier.offset(widget.x.dp, widget.y.dp)
                        .track(tracer, tag)
                    if (widget.text != null) {
                        Badge(modifier = mods, containerColor = container) {
                            Text(widget.text)
                        }
                    } else {
                        Badge(modifier = mods, containerColor = container)
                    }
                }
                widget.kind == "badged-box" -> {
                    // The upstream `BadgedBox`: the badge hangs at the
                    // anchor's top end corner — a dot `BadgeOffset`, a
                    // content badge `BadgeWithContentHorizontalOffset` /
                    // `BadgeWithContentVerticalOffset`.
                    val tag = "badged-box${badgedBoxes++}"
                    val container = widget.color?.let { schemeColor(it) }
                        ?: BadgeDefaults.containerColor
                    BadgedBox(
                        badge = {
                            // Same null-content rule as the `badge` widgets:
                            // a non-null lambda that draws nothing still
                            // counts as content upstream (`LargeSize`).
                            if (widget.text != null) {
                                Badge(containerColor = container) {
                                    Text(widget.text)
                                }
                            } else {
                                Badge(containerColor = container)
                            }
                        },
                        modifier = Modifier.offset(widget.x.dp, widget.y.dp)
                            .track(tracer, tag),
                    ) {
                        // The same `LocalContentColor` provision the `icon`
                        // widgets get — `onSurface` (the `on_background`
                        // stand-in on the Slint side, equal at the pin).
                        CompositionLocalProvider(
                            androidx.compose.material3.LocalContentColor provides
                                scheme.onSurface,
                        ) {
                            Icon(
                                sceneIcon(widget.icon ?: "check"),
                                contentDescription = null,
                            )
                        }
                    }
                }                widget.kind == "loading-indicator" ||
                    widget.kind == "contained-loading-indicator" -> {
                    // The 48dp indicator draws at the scene's declared
                    // coordinates on both sides.
                    Box(Modifier.offset(widget.x.dp, widget.y.dp)) {
                        if (widget.indeterminate) {
                            if (widget.kind == "loading-indicator") {
                                LoadingIndicator()
                            } else {
                                ContainedLoadingIndicator()
                            }
                        } else {
                            val progress = widget.progress
                            if (widget.kind == "loading-indicator") {
                                LoadingIndicator(progress = { progress })
                            } else {
                                ContainedLoadingIndicator(progress = { progress })
                            }
                        }
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
) = emitStateInteractions(
    widget.state,
    scene,
    tracer,
    elementId,
    interactionSource,
    emitPress,
    pressOffset,
    density,
    null,
)

/** The state-driven version — also usable per item (a connected-button
 * group's `items[].state`), where `onRelease` runs at the click's release
 * `at` time when this item was the one pressed (the single/multi-select
 * flip). */
@Composable
private fun emitStateInteractions(
    state: String,
    scene: Scene,
    tracer: Tracer,
    elementId: String,
    interactionSource: ReplayableInteractionSource,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
    pressOffset: Offset,
    density: Float,
    onRelease: Runnable?,
) {
    when (state) {
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
        // receive the press whenever they subscribe during the pump.
        //
        // Timed scenes still go through the
        // frame sink — the Slint driver dispatches //ACTION= just after its
        // own pre-press baseline frame, and an earlier emit would put
        // pressed ink (and the morph's start) into frame 0 — the
        // composition runs ahead of the pump during setup. Landing after
        // frame 0 also keeps the press off uptime 0, where a ripple's frame
        // callback would abort layoutlib.
        "pressed" -> {
            // `emit()` suspends until every subscriber has the emission —
            // deterministic where a frame-sink `tryEmit` is not: under a
            // multi-density record the second pump's composition schedules
            // the interaction collectors late for these composed-modifier
            // nodes and the buffered press is never picked up. Awaiting one
            // frame keeps the press off uptime 0 (a ripple's frame callback
            // there aborts layoutlib) while still landing the ink at the
            // same early moment the Slint driver dispatches `//ACTION=`.
            LaunchedEffect(Unit) {
                withFrameNanos { }
                interactionSource.emit(PressInteraction.Press(pressOffset))
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
    if (pressAction != null && state != "pressed") {
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
                emitted?.let {
                    interactionSource.tryEmit(PressInteraction.Release(it))
                    onRelease?.run()
                }
                emitted = null
            }
            DisposableEffect(release) {
                val entry = releaseAction.at to release
                emitPress.add(entry)
                onDispose { emitPress.remove(entry) }
            }
        }
    }
    // `move` actions drive hover the way the driver's pointer move does on
    // the Slint side: entering the widget's bounds emits
    // `HoverInteraction.Enter`, leaving them emits `Exit` — each at the
    // action's `at` time. Hit-tested like the press above so a pointer
    // inside another widget never lights this one up.
    scene.actions.filter { it.kind == "move" }.forEach { move ->
        var hoverEnter: HoverInteraction.Enter? = null
        val step = Runnable {
            val b = tracer.elementBounds[elementId]
            val inside = b == null ||
                (move.x * density >= b.left && move.x * density <= b.right &&
                    move.y * density >= b.top && move.y * density <= b.bottom)
            if (inside && hoverEnter == null) {
                val e = HoverInteraction.Enter()
                hoverEnter = e
                interactionSource.tryEmit(e)
            } else if (!inside) {
                hoverEnter?.let { interactionSource.tryEmit(HoverInteraction.Exit(it)) }
                hoverEnter = null
            }
        }
        DisposableEffect(step) {
            val entry = move.at to step
            emitPress.add(entry)
            onDispose { emitPress.remove(entry) }
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
    // Google Material Icon exports carry a viewport-sized `fill="none"`
    // bounds path that resvg skips — honoring the attribute keeps the
    // ImageVector identical to what the Slint side rasterizes.
    Regex("""<path[^>]*>""").findAll(svg).forEach { m ->
        val tag = m.value
        if (tag.contains("""fill="none"""")) return@forEach
        val d = Regex("""d="([^"]+)"""").find(tag)?.groupValues?.get(1)
            ?: return@forEach
        builder.addPath(
            androidx.compose.ui.graphics.vector.addPathNodes(d),
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
            "surface" -> scheme.surface
            "onSurface" -> scheme.onSurface
            "surfaceVariant" -> scheme.surfaceVariant
            "onSurfaceVariant" -> scheme.onSurfaceVariant
            "inverseSurface" -> scheme.inverseSurface
            "inverseOnSurface" -> scheme.inverseOnSurface
            "background" -> scheme.background
            "onBackground" -> scheme.onBackground
            "surfaceTint" -> scheme.surfaceTint
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

/** `switch` — a hoisted `checked` like every upstream selection control
 * (`Switch(checked, onCheckedChange, …)`), with a scene-side mirror of
 * the pin's private `ThumbNode` so `//TRACE_PROPS=thumb_size,thumb_offset`
 * reads the same value the Slint out properties animate. The targets
 * (`Switch.kt` L283-306: pressed → `PressedHandleWidth`, `hasContent ||
 * checked` → `ThumbDiameter`, else `UncheckedThumbDiameter`; pressed pulls
 * the thumb one `TrackOutlineWidth` off the far edge, `checked` sits at
 * `(SwitchWidth - ThumbDiameter) - ThumbPadding`) animate on
 * `SnapSpec` while pressed and `MotionSchemeKeyTokens.FastSpatial`
 * otherwise. `hasContent` measures the current state's slot — a
 * `thumbContent` that draws nothing while unchecked reports 0. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun StateSwitch(
    widget: Widget,
    scene: Scene,
    tracer: Tracer,
    elementId: String,
    density: Float,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
) {
    val interactionSource = remember { ReplayableInteractionSource() }
    var selected by remember { mutableStateOf(widget.checked) }
    val pressed by interactionSource.collectIsPressedAsState()
    val hasContent = widget.icon != null

    fun targetSize(): Float =
        if (pressed) 28f else if (hasContent || selected) 24f else 16f
    fun targetOffset(size: Float): Float =
        if (pressed && selected) 22f
        else if (pressed) 2f
        else if (selected) 24f
        else (32f - size) / 2f

    val thumbSize = remember { Animatable(
        if (hasContent || selected) 24f else 16f) }
    val thumbOffset = remember { Animatable(
        if (selected) 24f else (32f - (if (hasContent) 24f else 16f)) / 2f) }
    val spatialSpec = androidx.compose.material3.MaterialTheme.motionScheme.fastSpatialSpec<Float>()
    LaunchedEffect(selected, pressed) {
        val size = targetSize()
        val offset = targetOffset(size)
        val spec = if (pressed) androidx.compose.animation.core.snap<Float>() else spatialSpec
        launch { thumbSize.animateTo(size, spec) }
        launch { thumbOffset.animateTo(offset, spec) }
    }
    tracer.propGetters["thumb_size"] = { thumbSize.value.toDouble() }
    tracer.propGetters["thumb_offset"] = { thumbOffset.value.toDouble() }

    emitStateInteractions(
        widget.state,
        scene,
        tracer,
        elementId,
        interactionSource,
        emitPress,
        Offset(16f * density, 16f * density),
        density,
        Runnable { selected = !selected },
    )

    val iconVector = widget.icon?.let { sceneIcon(it) }
    Switch(
        checked = selected,
        onCheckedChange = { selected = it },
        modifier = Modifier.offset(widget.x.dp, widget.y.dp)
            .track(tracer, elementId),
        enabled = widget.enabled,
        interactionSource = interactionSource,
        thumbContent = if (iconVector != null) {
            { Icon(iconVector, contentDescription = null, modifier = Modifier.size(SwitchDefaults.IconSize)) }
        } else {
            null
        },
    )
}

/** `bottom_end` → `Alignment.BottomEnd` — `animateFloatingActionButton`'s
 * scale pivot. */
private fun fabAlignment(name: String): androidx.compose.ui.Alignment = when (name) {
    "top_start" -> androidx.compose.ui.Alignment.TopStart
    "top_center" -> androidx.compose.ui.Alignment.TopCenter
    "top_end" -> androidx.compose.ui.Alignment.TopEnd
    "center_start" -> androidx.compose.ui.Alignment.CenterStart
    "center" -> androidx.compose.ui.Alignment.Center
    "center_end" -> androidx.compose.ui.Alignment.CenterEnd
    "bottom_start" -> androidx.compose.ui.Alignment.BottomStart
    "bottom_center" -> androidx.compose.ui.Alignment.BottomCenter
    "bottom_end" -> androidx.compose.ui.Alignment.BottomEnd
    else -> error("unknown fab alignment $name")
}

/** A FAB family widget — `FloatingActionButton`/`ExtendedFloatingActionButton`
 * plus the expressive S/M/L sizes (and the deprecated small FAB). `variant`
 * selects the `FloatingActionButtonElevation` table (`lowered`,
 * `bottom-app-bar`); `color` pins `containerColor` (the upstream
 * `contentColorFor` default fills the content); `toggle` flips `expanded`
 * or `shown` on a scripted click; `label_width` is `Modifier.width` on the
 * text slot. `Modifier.animateFloatingActionButton` is caller-applied
 * upstream — `shown`/`alignment`/`target_scale` land on the outer
 * modifier. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun StateFab(
    widget: Widget,
    scene: Scene,
    tracer: Tracer,
    textId: String,
    elementId: String,
    density: Float,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
) {
    val interactionSource = remember { ReplayableInteractionSource() }
    emitStateInteractions(
        widget,
        scene,
        tracer,
        elementId,
        interactionSource,
        emitPress,
        Offset(20f * density, 16f * density),
        density,
    )

    var expanded by remember { mutableStateOf(widget.expanded) }
    var shown by remember { mutableStateOf(widget.shown) }
    if (widget.toggle != null && sceneActionsClick(scene)) {
        DisposableEffect(Unit) {
            val flip = Runnable {
                when (widget.toggle) {
                    "expanded" -> expanded = !expanded
                    "shown" -> shown = !shown
                }
            }
            // The click completes on release — the state flips at its `at`.
            val at = scene.actions.firstOrNull { it.kind == "release" }?.at ?: 0L
            val entry = at to flip
            emitPress.add(entry)
            onDispose { emitPress.remove(entry) }
        }
    }

    val elevation = when (widget.variant) {
        "lowered" -> FloatingActionButtonDefaults.loweredElevation()
        "bottom-app-bar" -> FloatingActionButtonDefaults.bottomAppBarFabElevation()
        else -> FloatingActionButtonDefaults.elevation()
    }
    val containerColor = widget.color?.let { schemeColor(it) }
        ?: FloatingActionButtonDefaults.containerColor

    val modifier = Modifier.offset(widget.x.dp, widget.y.dp)
        .animateFloatingActionButton(
            visible = shown,
            alignment = fabAlignment(widget.alignment),
            targetScale = widget.targetScale,
        )
        .track(tracer, elementId)

    // Live-value probes replicating the composables' internal animatables —
    // the same approach as the buttons' `container_radius` probe.
    val motionScheme = androidx.compose.material3.MaterialTheme.motionScheme
    if (widget.kind == "fab") {
        // `animateFloatingActionButton`'s two animatables: scale on the
        // fast spatial spec, alpha on the fast effects spec.
        val scaleT by animateFloatAsState(
            targetValue = if (shown) 1f else 0f,
            animationSpec = motionScheme.fastSpatialSpec(),
            label = "show_scale",
        )
        val alphaT by animateFloatAsState(
            targetValue = if (shown) 1f else 0f,
            animationSpec = motionScheme.fastEffectsSpec(),
            label = "show_alpha",
        )
        tracer.propGetters["show_scale"] = { scaleT.toDouble() }
        tracer.propGetters["show_alpha"] = { alphaT.toDouble() }
    } else {
        // S/M/L: `updateTransition(expanded ? 1f : 0f)` — FastSpatial width
        // lerp, FastEffects label alpha. Baseline: `AnimatedVisibility`
        // expands the slot on FastSpatial and fades it — DefaultEffects in,
        // FastEffects out — so the probes pick the spec by direction.
        val expandT by animateFloatAsState(
            targetValue = if (expanded) 1f else 0f,
            animationSpec = if (widget.size == "baseline" && !expanded) {
                motionScheme.defaultSpatialSpec()
            } else {
                motionScheme.fastSpatialSpec()
            },
            label = "expand_progress",
        )
        val alphaT by animateFloatAsState(
            targetValue = if (expanded) 1f else 0f,
            animationSpec = if (widget.size == "baseline" && expanded) {
                motionScheme.defaultEffectsSpec()
            } else {
                motionScheme.fastEffectsSpec()
            },
            label = "label_alpha",
        )
        tracer.propGetters["expand_progress"] = { expandT.toDouble() }
        tracer.propGetters["label_alpha"] = { alphaT.toDouble() }
    }

    // `shadow_elevation` probe — mirrors `FloatingActionButtonElevationAnimatable`
    // (FloatingActionButton.kt at the pin) on the same interaction stream.
    // `propGetters` keys are flat, so only `button0` registers — the same
    // element the Slint side forwards to its case root.
    val shadowElevation =
        fabShadowElevationProbe(interactionSource, fabElevationLevels(widget.variant))
    if (elementId == "button0") {
        tracer.propGetters["shadow_elevation"] = { shadowElevation.value.toDouble() }
    }

    // The icon inside a FAB is caller content — upstream callers size it to
    // the recommended edge (`FabBaselineTokens.IconSize`/`FabSmallTokens`
    // 24, `FloatingActionButtonDefaults.MediumIconSize`/`FabMediumTokens` 28,
    // `LargeIconSize` 36 — the hard-coded value, `FabLargeTokens.IconSize`
    // marked incorrect upstream; `ExtendedFab*Tokens.IconSize` 24/24/28/32).
    val iconEdge =
        if (widget.kind == "fab") {
            when (widget.size) {
                "medium" -> 28.dp
                "large" -> 36.dp
                else -> 24.dp
            }
        } else {
            when (widget.size) {
                "medium" -> 28.dp
                "large" -> 32.dp
                else -> 24.dp
            }
        }
    if (widget.kind == "fab") {
        val content: @Composable () -> Unit = {
            Icon(
                sceneIcon(widget.icon ?: "check"),
                contentDescription = widget.text,
                modifier = Modifier.size(iconEdge),
            )
        }
        when (widget.size) {
            "small" -> SmallFloatingActionButton(
                onClick = {},
                modifier = modifier,
                containerColor = containerColor,
                elevation = elevation,
                interactionSource = interactionSource,
                content = content,
            )
            "medium" -> MediumFloatingActionButton(
                onClick = {},
                modifier = modifier,
                containerColor = containerColor,
                elevation = elevation,
                interactionSource = interactionSource,
                content = content,
            )
            "large" -> LargeFloatingActionButton(
                onClick = {},
                modifier = modifier,
                containerColor = containerColor,
                elevation = elevation,
                interactionSource = interactionSource,
                content = content,
            )
            else -> FloatingActionButton(
                onClick = {},
                modifier = modifier,
                containerColor = containerColor,
                elevation = elevation,
                interactionSource = interactionSource,
                content = content,
            )
        }
    } else {
        val labelModifier =
            if (widget.labelWidth > 0f) Modifier.width(widget.labelWidth.dp) else Modifier
        val icon = widget.icon
        if (icon == null) {
            // The text-only overloads take a `RowScope` content lambda.
            val content:
                @Composable androidx.compose.foundation.layout.RowScope.() -> Unit = {
                    Text(
                        widget.text ?: "",
                        modifier = labelModifier.then(
                            Modifier.trackText(tracer, textId, density),
                        ),
                        onTextLayout = recordTextLayout(
                            tracer,
                            textId,
                            androidx.compose.ui.platform.LocalDensity.current,
                            androidx.compose.ui.platform.LocalFontFamilyResolver.current,
                            androidx.compose.material3.MaterialTheme.typography.labelLarge.fontFamily,
                        ),
                    )
                }
            when (widget.size) {
                "small" -> SmallExtendedFloatingActionButton(
                    onClick = {},
                    modifier = modifier,
                    containerColor = containerColor,
                    elevation = elevation,
                    interactionSource = interactionSource,
                    content = content,
                )
                "medium" -> MediumExtendedFloatingActionButton(
                    onClick = {},
                    modifier = modifier,
                    containerColor = containerColor,
                    elevation = elevation,
                    interactionSource = interactionSource,
                    content = content,
                )
                "large" -> LargeExtendedFloatingActionButton(
                    onClick = {},
                    modifier = modifier,
                    containerColor = containerColor,
                    elevation = elevation,
                    interactionSource = interactionSource,
                    content = content,
                )
                else -> ExtendedFloatingActionButton(
                    onClick = {},
                    modifier = modifier,
                    containerColor = containerColor,
                    elevation = elevation,
                    interactionSource = interactionSource,
                    content = content,
                )
            }
        } else {
            val text: @Composable () -> Unit = {
                Text(
                    widget.text ?: "",
                    modifier = labelModifier.then(
                        Modifier.trackText(tracer, textId, density),
                    ),
                    onTextLayout = recordTextLayout(
                        tracer,
                        textId,
                        androidx.compose.ui.platform.LocalDensity.current,
                        androidx.compose.ui.platform.LocalFontFamilyResolver.current,
                        androidx.compose.material3.MaterialTheme.typography.labelLarge.fontFamily,
                    ),
                )
            }
            val iconContent: @Composable () -> Unit = {
                Icon(sceneIcon(icon), contentDescription = null, modifier = Modifier.size(iconEdge))
            }
            when (widget.size) {
                "small" -> SmallExtendedFloatingActionButton(
                    text = text,
                    icon = iconContent,
                    onClick = {},
                    modifier = modifier,
                    expanded = expanded,
                    containerColor = containerColor,
                    elevation = elevation,
                    interactionSource = interactionSource,
                )
                "medium" -> MediumExtendedFloatingActionButton(
                    text = text,
                    icon = iconContent,
                    onClick = {},
                    modifier = modifier,
                    expanded = expanded,
                    containerColor = containerColor,
                    elevation = elevation,
                    interactionSource = interactionSource,
                )
                "large" -> LargeExtendedFloatingActionButton(
                    text = text,
                    icon = iconContent,
                    onClick = {},
                    modifier = modifier,
                    expanded = expanded,
                    containerColor = containerColor,
                    elevation = elevation,
                    interactionSource = interactionSource,
                )
                else -> ExtendedFloatingActionButton(
                    text = text,
                    icon = iconContent,
                    onClick = {},
                    modifier = modifier,
                    expanded = expanded,
                    containerColor = containerColor,
                    elevation = elevation,
                    interactionSource = interactionSource,
                )
            }
        }
    }
}

/** The four dp levels a `FloatingActionButtonElevation` variant carries, in
 * the order upstream's constructor takes them — the same values
 * `FloatingActionButtonDefaults.elevation()`/`loweredElevation()`/
 * `bottomAppBarFabElevation()` default to (FloatingActionButton.kt at the
 * pin: `FabPrimaryContainerTokens` L3/L3/L3/L4, `ElevationTokens` L1/L1/L1/L2,
 * flat 0 for bottom-app-bar). */
private data class FabElevationLevels(
    val defaultElevation: Dp,
    val pressedElevation: Dp,
    val focusedElevation: Dp,
    val hoveredElevation: Dp,
)

private fun fabElevationLevels(variant: String?): FabElevationLevels =
    when (variant) {
        "lowered" -> FabElevationLevels(1.dp, 1.dp, 1.dp, 3.dp)
        "bottom-app-bar" -> FabElevationLevels(0.dp, 0.dp, 0.dp, 0.dp)
        else -> FabElevationLevels(6.dp, 6.dp, 6.dp, 8.dp)
    }

/** Live-value probe replicating `FloatingActionButtonElevationAnimatable`
 * (FloatingActionButton.kt at the pin) on the same `interactionSource` the
 * composable animates its shadow with: the last interaction wins; `to` runs
 * `DefaultIncomingSpec` (120 ms, `FastOutSlowInEasing`), `to == null` runs
 * the outgoing spec `Elevation.kt` picks for `from` — 120 ms for hover,
 * 150 ms for press/focus, both `CubicBezierEasing(0.4, 0, 0.6, 1)`. */
@Composable
private fun fabShadowElevationProbe(
    interactionSource: androidx.compose.foundation.interaction.InteractionSource,
    levels: FabElevationLevels,
): State<Float> {
    // Animating the dp value as a float — `Dp.VectorConverter` animates the
    // same scalar, so the trajectory is identical.
    val animatable =
        remember(interactionSource) {
            Animatable(levels.defaultElevation.value, Float.VectorConverter)
        }
    var lastTargetInteraction by remember { mutableStateOf<Interaction?>(null) }

    fun Interaction?.targetElevation(): Float =
        when (this) {
            is PressInteraction.Press -> levels.pressedElevation.value
            is HoverInteraction.Enter -> levels.hoveredElevation.value
            is FocusInteraction.Focus -> levels.focusedElevation.value
            else -> levels.defaultElevation.value
        }

    LaunchedEffect(interactionSource) {
        val interactions = mutableListOf<Interaction>()
        interactionSource.interactions.collect { interaction ->
            when (interaction) {
                is HoverInteraction.Enter -> interactions.add(interaction)
                is HoverInteraction.Exit -> interactions.remove(interaction.enter)
                is FocusInteraction.Focus -> interactions.add(interaction)
                is FocusInteraction.Unfocus -> interactions.remove(interaction.focus)
                is PressInteraction.Press -> interactions.add(interaction)
                is PressInteraction.Release,
                is PressInteraction.Cancel,
                -> interactions.remove(
                    if (interaction is PressInteraction.Release) {
                        interaction.press
                    } else {
                        (interaction as PressInteraction.Cancel).press
                    },
                )
            }
            val to = interactions.lastOrNull()
            val from = lastTargetInteraction
            lastTargetInteraction = to
            val target = to.targetElevation()
            if (animatable.targetValue != target) {
                launch {
                    val spec =
                        when {
                            to != null -> TweenSpec<Float>(120, easing = FastOutSlowInEasing)
                            from is HoverInteraction.Enter ||
                                from is PressInteraction.Press ||
                                from is DragInteraction.Start ||
                                from is FocusInteraction.Focus ->
                                TweenSpec(
                                    if (from is HoverInteraction.Enter) 120 else 150,
                                    easing = CubicBezierEasing(0.4f, 0f, 0.6f, 1f),
                                )
                            else -> null
                        }
                    if (spec != null) animatable.animateTo(target, spec)
                    else animatable.snapTo(target)
                }
            }
        }
    }
    return animatable.asState()
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
): androidx.compose.runtime.State<Boolean> = pressInkMarker(widget.state, emitPress)

/** The state-driven version — usable per group item. */
@Composable
private fun pressInkMarker(
    state: String,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
): androidx.compose.runtime.State<Boolean> {
    val show = remember { mutableStateOf(false) }
    if (state != "pressed") {
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

/** A `ListItem`/`SegmentedListItem` parity widget. The overload dispatch
 * mirrors the scene fields: `checkable` → `onCheckedChange`, `selectable`
 * → `selected`+`onClick`, `interactive` → plain `onClick`, else the
 * non-interactive base overload. On `selectable`/`checkable` the host owns
 * the state — a press+release action pair is the click, flipping
 * `selected`/`checked` at the release's `at` time like the Slint case's
 * `clicked` handler. `container_radius` probes the shape morph at
 * `MotionScheme.fastSpatialSpec` — `ListItemShapes`' `shapeAnimationSpec`
 * — following pressed → selected → resting; focus, hover, and `dragged`
 * never reach a scene (no scene action emits focus, and platform shadows
 * deadlock layoutlib, so no scene asserts a dragged item's Level4
 * elevation). */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun StateListItem(
    widget: Widget,
    scene: Scene,
    tracer: Tracer,
    textBase: Int,
    elementId: String,
    density: Float,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
) {
    val interactionSource = remember { ReplayableInteractionSource() }
    val interactive = widget.interactive || widget.selectable || widget.checkable
    if (interactive) {
        emitStateInteractions(
            widget, scene, tracer, elementId, interactionSource, emitPress,
            Offset(40f * density, 20f * density), density,
        )
    }

    var checked by remember { mutableStateOf(widget.checked) }
    var selected by remember { mutableStateOf(widget.selected) }
    val clickToggles = (widget.checkable || widget.selectable) && sceneActionsClick(scene)
    DisposableEffect(Unit) {
        val flip = Runnable { if (widget.checkable) checked = !checked else selected = !selected }
        val entry =
            (scene.actions.firstOrNull { it.kind == "release" }?.at ?: 0L) to flip
        if (clickToggles) emitPress.add(entry)
        onDispose { emitPress.remove(entry) }
    }
    val selectedEffective =
        if (widget.selectable) selected else if (widget.checkable) checked else false

    val segmented = widget.kind == "segmented-list-item"
    val shapes = if (segmented) {
        ListItemDefaults.segmentedShapes(widget.index, widget.count)
    } else {
        ListItemDefaults.shapes()
    }
    val colors =
        if (segmented) ListItemDefaults.segmentedColors() else ListItemDefaults.colors()

    // `container_radius` — every list container corner is an absolute-dp
    // token, so `radiusOf`'s height argument is inert (it only resolves
    // percent corners, which list shapes never use).
    val pressed by interactionSource.collectIsPressedAsState()
    val radius by animateFloatAsState(
        targetValue = radiusOf(
            when {
                pressed -> shapes.pressedShape
                selectedEffective -> shapes.selectedShape
                else -> shapes.shape
            },
            56.dp,
            density,
        ),
        animationSpec =
            androidx.compose.material3.MaterialTheme.motionScheme.fastSpatialSpec<Float>(),
        label = "container_radius",
    )
    tracer.propGetters["container_radius"] = { radius.toDouble() }

    var modifier = Modifier.offset(widget.x.dp, widget.y.dp)
    if (widget.width > 0) {
        modifier = modifier.width(widget.width.dp)
    }
    modifier = modifier.track(tracer, elementId)

    val resolver = androidx.compose.ui.platform.LocalFontFamilyResolver.current
    val localDensity = LocalDensity.current
    // `text:<n>` ids follow the Slint side's Text element order: the avatar
    // label sits in the leading slot before the column's overline/headline/
    // supporting and the row's trailing text.
    var tid = textBase
    fun nextTextId() = "text:${tid++}"
    fun slotText(id: String, t: String, color: Color): @Composable () -> Unit = {
        Text(
            text = t,
            color = color,
            modifier = Modifier.trackText(tracer, id, density),
            onTextLayout =
                recordTextLayout(tracer, id, localDensity, resolver, null),
        )
    }
    // The leading slot mirrors the Slint `ListTile` slots: the 40px avatar
    // circle (`ItemLeadingAvatarColor`/`ItemLeadingAvatarLabelColor`), a
    // 56px `leading_image` clipped to `ItemLeadingImageExpressiveShape`
    // (`corner_small` = 8dp), or the 24px `leading_icon`.
    val leading: (@Composable () -> Unit)? = when {
        widget.avatar != null -> {
            val avatarSlot = slotText(
                nextTextId(),
                widget.avatar!!,
                schemeColor("on-primary-container"),
            )
            val slot: @Composable () -> Unit = {
                Box(
                    Modifier.size(40.dp)
                        .clip(androidx.compose.foundation.shape.CircleShape)
                        .background(schemeColor("primary-container")),
                    contentAlignment = androidx.compose.ui.Alignment.Center,
                ) {
                    avatarSlot()
                }
            }
            slot
        }
        widget.leadingImage != null -> {
            {
                androidx.compose.foundation.Image(
                    imageVector = sceneIcon(widget.leadingImage!!),
                    contentDescription = null,
                    modifier = Modifier.size(56.dp)
                        .clip(androidx.compose.foundation.shape.RoundedCornerShape(8.dp)),
                    contentScale = androidx.compose.ui.layout.ContentScale.FillBounds,
                )
            }
        }
        widget.icon != null -> {
            {
                Icon(
                    sceneIcon(widget.icon!!),
                    contentDescription = null,
                    modifier = Modifier.size(24.dp),
                )
            }
        }
        else -> null
    }
    val overline: (@Composable () -> Unit)? = widget.overline?.let { t ->
        slotText(nextTextId(), t, Color.Unspecified)
    }
    val content: @Composable () -> Unit =
        slotText(nextTextId(), widget.text ?: "", Color.Unspecified)
    val supporting: (@Composable () -> Unit)? = widget.supporting?.let { t ->
        slotText(nextTextId(), t, Color.Unspecified)
    }
    val trailing: (@Composable () -> Unit)? = when {
        widget.trailingIcon != null -> {
            {
                Icon(
                    sceneIcon(widget.trailingIcon!!),
                    contentDescription = null,
                    modifier = Modifier.size(24.dp),
                )
            }
        }
        widget.trailingText != null ->
            slotText(nextTextId(), widget.trailingText!!, Color.Unspecified)
        else -> null
    }

    if (segmented) {
        when {
            widget.checkable -> SegmentedListItem(
                checked = checked,
                onCheckedChange = { checked = it },
                shapes = shapes,
                modifier = modifier,
                enabled = widget.enabled,
                leadingContent = leading,
                trailingContent = trailing,
                overlineContent = overline,
                supportingContent = supporting,
                colors = colors,
                interactionSource = interactionSource,
                content = content,
            )
            widget.selectable -> SegmentedListItem(
                selected = selected,
                onClick = { selected = !selected },
                shapes = shapes,
                modifier = modifier,
                enabled = widget.enabled,
                leadingContent = leading,
                trailingContent = trailing,
                overlineContent = overline,
                supportingContent = supporting,
                colors = colors,
                interactionSource = interactionSource,
                content = content,
            )
            interactive -> SegmentedListItem(
                onClick = {},
                shapes = shapes,
                modifier = modifier,
                enabled = widget.enabled,
                leadingContent = leading,
                trailingContent = trailing,
                overlineContent = overline,
                supportingContent = supporting,
                colors = colors,
                interactionSource = interactionSource,
                content = content,
            )
            else -> SegmentedListItem(
                // material3 1.5.0-alpha18 ships no non-interactive
                // SegmentedListItem overload; a no-op onClick draws the
                // identical resting visual (ripple only appears on touch,
                // and no scene action touches a non-interactive item).
                onClick = {},
                shapes = shapes,
                modifier = modifier,
                enabled = widget.enabled,
                leadingContent = leading,
                trailingContent = trailing,
                overlineContent = overline,
                supportingContent = supporting,
                colors = colors,
                interactionSource = interactionSource,
                content = content,
            )
        }
    } else {
        when {
            widget.checkable -> ListItem(
                checked = checked,
                onCheckedChange = { checked = it },
                modifier = modifier,
                enabled = widget.enabled,
                leadingContent = leading,
                trailingContent = trailing,
                overlineContent = overline,
                supportingContent = supporting,
                shapes = shapes,
                colors = colors,
                interactionSource = interactionSource,
                content = content,
            )
            widget.selectable -> ListItem(
                selected = selected,
                onClick = { selected = !selected },
                modifier = modifier,
                enabled = widget.enabled,
                leadingContent = leading,
                trailingContent = trailing,
                overlineContent = overline,
                supportingContent = supporting,
                shapes = shapes,
                colors = colors,
                interactionSource = interactionSource,
                content = content,
            )
            interactive -> ListItem(
                onClick = {},
                modifier = modifier,
                enabled = widget.enabled,
                leadingContent = leading,
                trailingContent = trailing,
                overlineContent = overline,
                supportingContent = supporting,
                shapes = shapes,
                colors = colors,
                interactionSource = interactionSource,
                content = content,
            )
            else -> ListItem(
                // The legacy `headlineContent` overload is upstream's only
                // non-interactive ListItem: flat `ListItemDefaults.shape`
                // corners, no `enabled` — it cannot render a disabled item.
                headlineContent = content,
                modifier = modifier,
                leadingContent = leading,
                trailingContent = trailing,
                overlineContent = overline,
                supportingContent = supporting,
                colors = colors,
            )
        }
    }
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

/** `ButtonGroupDefaults.connected{Leading,Middle,Trailing}ButtonShapes()`
 * for a `connected-button`'s `position`, or the `VerticalButtonGroupSample`
 * shapes: the middle shape with `CornerSize(100)` caps on the first/last
 * item and the uniform `ToggleButtonDefaults.pressedShape` (6dp). */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun connectedShapesFor(position: String, vertical: Boolean): ToggleButtonShapes {
    if (vertical) {
        val middle = ButtonGroupDefaults.connectedMiddleButtonShapes()
        val resting = (middle.shape as RoundedCornerShape).let {
            when (position) {
                "start" -> it.copy(topStart = CornerSize(100), topEnd = CornerSize(100))
                "end" -> it.copy(bottomStart = CornerSize(100), bottomEnd = CornerSize(100))
                else -> it
            }
        }
        return ToggleButtonShapes(
            shape = resting,
            pressedShape = ToggleButtonDefaults.pressedShape,
            checkedShape = ButtonGroupDefaults.connectedButtonCheckedShape,
        )
    }
    return when (position) {
        "start" -> ButtonGroupDefaults.connectedLeadingButtonShapes()
        "end" -> ButtonGroupDefaults.connectedTrailingButtonShapes()
        else -> ButtonGroupDefaults.connectedMiddleButtonShapes()
    }
}

/** One corner of a token-backed `RoundedCornerShape` in dp — `radiusOf`'s
 * per-corner counterpart for the connected-button morph trace. `h` is the
 * container height: always the smaller dimension here, so `Size(h,h)`
 * reproduces `PercentCornerSize`'s `minDimension` exactly. */
private fun cornerRadiusOf(corner: androidx.compose.foundation.shape.CornerSize,
    h: androidx.compose.ui.unit.Dp, density: Float): Float {
    val boxPx = h.value * density
    return corner.toPx(
        androidx.compose.ui.geometry.Size(boxPx, boxPx),
        androidx.compose.ui.unit.Density(density),
    ) / density
}

/** The live per-corner morph radii (dp) of a connected item's shape —
 * upstream's internal `AnimatedShapeState` animates each corner
 * separately; these four `animateFloatAsState`s reproduce it for the
 * `corner_*` trace props and the press-ink clip. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun connectedCorners(
    shapes: ToggleButtonShapes,
    pressed: Boolean,
    checked: Boolean,
    h: Float,
    density: Float,
): List<Float> {
    val spec = androidx.compose.material3.MaterialTheme.motionScheme.fastSpatialSpec<Float>()
    val active = when {
        pressed -> shapes.pressedShape
        checked -> shapes.checkedShape
        else -> shapes.shape
    }
    require(active is RoundedCornerShape) { "connected shapes are RoundedCornerShape, got $active" }
    val box = androidx.compose.ui.unit.Dp(h)
    val tl by animateFloatAsState(
        cornerRadiusOf(active.topStart, box, density), spec, label = "corner_tl",
    )
    val tr by animateFloatAsState(
        cornerRadiusOf(active.topEnd, box, density), spec, label = "corner_tr",
    )
    val br by animateFloatAsState(
        cornerRadiusOf(active.bottomEnd, box, density), spec, label = "corner_br",
    )
    val bl by animateFloatAsState(
        cornerRadiusOf(active.bottomStart, box, density), spec, label = "corner_bl",
    )
    return listOf(tl, tr, br, bl)
}

/** A `ConnectedButton` — one `ToggleButton` of a connected button group —
 * in the interaction state the scene asks for. Exposes the per-corner
 * morph values as `corner_top_left`/`corner_top_right`/
 * `corner_bottom_right`/`corner_bottom_left` trace props. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun StateConnectedButton(
    widget: Widget,
    scene: Scene,
    tracer: Tracer,
    textId: String,
    elementId: String,
    density: Float,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
) {
    val interactionSource = remember { ReplayableInteractionSource() }
    emitStateInteractions(
        widget,
        scene,
        tracer,
        elementId,
        interactionSource,
        emitPress,
        Offset(40f * density, 20f * density),
        density,
    )

    var checked by remember { mutableStateOf(widget.checked) }
    val clickToggles = sceneActionsClick(scene)
    DisposableEffect(Unit) {
        val flip = Runnable { checked = !checked }
        val at = scene.actions.firstOrNull { it.kind == "release" }?.at ?: 0L
        val entry = at to flip
        if (clickToggles) {
            emitPress.add(entry)
        }
        onDispose { emitPress.remove(entry) }
    }

    val h = ToggleButtonDefaults.MinHeight
    val shapes = connectedShapesFor(widget.position, widget.vertical)
    val modifier = Modifier.offset(widget.x.dp, widget.y.dp)
        .height(h)
        .then(if (widget.width > 0f) Modifier.width(widget.width.dp) else Modifier)
        .track(tracer, elementId)
    val iconVector = widget.icon?.let { sceneIcon(it) }
    val checkedIconVector = widget.icon?.let { sceneIcon(widget.checkedIcon ?: it) }
    // The pin's `ToggleButton(icon=)` slot sizes the icon through
    // `iconSizeFor` (20dp at 40dp height); alpha18's composable predates
    // that slot, so the size is pinned on the `Icon` itself.
    val labelStyle = ButtonDefaults.textStyleFor(h)
    val content: @Composable androidx.compose.foundation.layout.RowScope.() -> Unit = {
        if (iconVector != null) {
            Icon(
                if (checked) checkedIconVector!! else iconVector,
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

    val pressed by interactionSource.collectIsPressedAsState()
    val (tl, tr, br, bl) = connectedCorners(shapes, pressed, checked, h.value, density)
    tracer.propGetters["corner_top_left"] = { tl.toDouble() }
    tracer.propGetters["corner_top_right"] = { tr.toDouble() }
    tracer.propGetters["corner_bottom_right"] = { br.toDouble() }
    tracer.propGetters["corner_bottom_left"] = { bl.toDouble() }

    val pressInk = pressInkMarker(widget, emitPress)

    ToggleButton(
        checked = checked,
        onCheckedChange = {},
        modifier = modifier,
        enabled = widget.enabled,
        shapes = shapes,
        contentPadding = ButtonDefaults.contentPaddingFor(h),
        interactionSource = interactionSource,
        content = content,
    )

    if (pressInk.value) {
        PressInkOverlay(
            widget,
            tracer,
            elementId,
            density,
            RoundedCornerShape(
                topStart = tl.dp,
                topEnd = tr.dp,
                bottomEnd = br.dp,
                bottomStart = bl.dp,
            ),
            connectedInkColor(checked),
        )
    }
}

/** The content color the ink tints — `ToggleButtonDefaults.toggleButtonColors()`. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun connectedInkColor(checked: Boolean): Color =
    ToggleButtonDefaults.toggleButtonColors()
        .let { if (checked) it.checkedContentColor else it.contentColor }

/** A `connected-button-group`/`vertical-connected-button-group` — the
 * upstream samples' `FlowRow`/`Column` of `ToggleButton`s with the
 * position's connected shapes. Items are tagged `group{n}item{i}`
 * (Compose-side only — repeater items carry no Slint id) so the press
 * action's hit-test reaches the right `InteractionSource`. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun StateConnectedGroup(
    widget: Widget,
    scene: Scene,
    tracer: Tracer,
    tag: String,
    density: Float,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
    textBase: Int,
) {
    val vertical = widget.kind == "vertical-connected-button-group"
    var selectedIndex by remember { mutableStateOf(widget.selectedIndex) }
    val checkedStates = remember {
        mutableStateListOf(*widget.items.map { it.checked }.toTypedArray())
    }
    val containerModifier = Modifier.offset(widget.x.dp, widget.y.dp)
        .then(if (widget.width > 0f) Modifier.width(widget.width.dp) else Modifier)
        .track(tracer, tag)
    val lastIndex = widget.items.size - 1

    val itemsContent: @Composable () -> Unit = {
        widget.items.forEachIndexed { index, item ->
            val position = when {
                index == 0 -> "start"
                index == lastIndex -> "end"
                else -> "middle"
            }
            val itemTag = "${tag}item$index"
            val interactionSource = remember { ReplayableInteractionSource() }
            if (!item.disabled) {
                emitStateInteractions(
                    item.state,
                    scene,
                    tracer,
                    itemTag,
                    interactionSource,
                    emitPress,
                    Offset(40f * density, 20f * density),
                    density,
                    Runnable {
                        if (widget.multiSelect) {
                            checkedStates[index] = !checkedStates[index]
                        } else {
                            selectedIndex = index
                        }
                    },
                )
            }
            val checked = if (widget.multiSelect) checkedStates[index] else selectedIndex == index
            val h = ToggleButtonDefaults.MinHeight
            val shapes = connectedShapesFor(position, vertical)
            val iconVector = item.icon?.let { sceneIcon(it) }
            val checkedIconVector = item.icon?.let { sceneIcon(item.checkedIcon ?: it) }
            val labelStyle = ButtonDefaults.textStyleFor(h)
            val pressed by interactionSource.collectIsPressedAsState()
            val (tl, tr, br, bl) = connectedCorners(shapes, pressed, checked, h.value, density)
            val pressInk = pressInkMarker(item.state, emitPress)

            // The Box keeps the ink overlay out of the FlowRow/Column's
            // child count — the scene-level PressInkOverlay offsets from
            // root bounds, which only composes correctly outside a layout.
            Box(Modifier.track(tracer, itemTag)) {
                ToggleButton(
                    checked = checked,
                    onCheckedChange = {},
                    // The samples' single-select items carry Role.RadioButton;
                    // multi-select keeps ToggleButton's own Role.Checkbox.
                    modifier =
                        if (widget.multiSelect) Modifier
                        else Modifier.semantics { role = Role.RadioButton },
                    enabled = !item.disabled,
                    shapes = shapes,
                    contentPadding = ButtonDefaults.contentPaddingFor(h),
                    interactionSource = interactionSource,
                ) {
                    if (iconVector != null) {
                        Icon(
                            if (checked) checkedIconVector!! else iconVector,
                            contentDescription = null,
                            modifier = Modifier.size(ButtonDefaults.iconSizeFor(h)),
                        )
                        Spacer(Modifier.width(ButtonDefaults.iconSpacingFor(h)))
                    }
                    val textId = "text:${textBase + index}"
                    Text(
                        item.text ?: "",
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

                if (pressInk.value) {
                    Box(
                        Modifier.matchParentSize()
                            .clip(
                                RoundedCornerShape(
                                    topStart = tl.dp,
                                    topEnd = tr.dp,
                                    bottomEnd = br.dp,
                                    bottomStart = bl.dp,
                                ),
                            )
                            .background(
                                connectedInkColor(checked)
                                    .copy(alpha = PRESSED_STATE_LAYER_ALPHA),
                            ),
                    )
                }
            }
        }
    }

    if (vertical) {
        // `Column(verticalArrangement = spacedBy((-6).dp))` — each item
        // pulls up 6dp over the previous one; later items paint over.
        Column(
            modifier = containerModifier,
            verticalArrangement = Arrangement.spacedBy((-6).dp),
        ) {
            itemsContent()
        }
    } else {
        FlowRow(
            modifier = containerModifier,
            horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            itemsContent()
        }
    }
}

/** `*-chip` widgets: the flat/elevated assist, filter, input and
 * suggestion chips. `variant: "expressive"` selects the `shapes:` overload
 * (corner morphing) on filter/input chips; `checked` is the composable's
 * `selected`, flipped at a click's release like the toggle buttons. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun StateChip(
    widget: Widget,
    scene: Scene,
    tracer: Tracer,
    textId: String,
    elementId: String,
    density: Float,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
) {
    val interactionSource = remember { ReplayableInteractionSource() }
    emitStateInteractions(widget, scene, tracer, elementId, interactionSource, emitPress, Offset(40f * density, 16f * density), density)

    // Filter and input chips are `selected:` composables — intrinsically
    // checkable; assist and suggestion chips are plain buttons.
    val checkable =
        widget.checkable ||
            widget.kind == "filter-chip" ||
            widget.kind == "elevated-filter-chip" ||
            widget.kind == "input-chip"
    var selected by remember { mutableStateOf(widget.checked) }
    val clickToggles = checkable && sceneActionsClick(scene)
    DisposableEffect(Unit) {
        val flip = Runnable { selected = !selected }
        val at = scene.actions.firstOrNull { it.kind == "release" }?.at ?: 0L
        val entry = at to flip
        if (clickToggles) {
            emitPress.add(entry)
        }
        onDispose { emitPress.remove(entry) }
    }

    val modifier = Modifier.offset(widget.x.dp, widget.y.dp)
        .then(if (widget.width > 0f) Modifier.width(widget.width.dp) else Modifier)
        .track(tracer, elementId)

    val labelStyle = androidx.compose.material3.MaterialTheme.typography.labelLarge
    val label: @Composable () -> Unit = {
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
    // `checkedIcon` swaps the leading slot while selected — the upstream
    // samples' `leadingIcon = if (selected) { check } else { icon }`.
    val leadingIconName = if (selected && widget.checkedIcon != null) widget.checkedIcon else widget.icon
    val leadingIcon: (@Composable () -> Unit)? = leadingIconName?.let { name ->
        { Icon(sceneIcon(name), contentDescription = null, modifier = Modifier.size(AssistChipDefaults.IconSize)) }
    }
    val trailingIcon: (@Composable () -> Unit)? = widget.trailingIcon?.let { name ->
        { Icon(sceneIcon(name), contentDescription = null, modifier = Modifier.size(FilterChipDefaults.IconSize)) }
    }
    val avatar: (@Composable () -> Unit)? = widget.avatar?.let { name ->
        { Icon(sceneIcon(name), contentDescription = null, modifier = Modifier.size(InputChipDefaults.AvatarSize)) }
    }
    val expressive = widget.variant == "expressive"

    // `shadow_elevation`/`corner_top_left` probes — the same Elevation.kt /
    // `shapeByInteraction` mechanics the FAB and toggle probes mirror.
    // Flat chips sit at 0 (the flat filter chip's table uses the
    // selected-hover token — 1dp — regardless of `selected`); elevated
    // chips run 1/1/1/3; disabled chips snap to their disabled level (0 in
    // these scenes). `propGetters` keys are flat, so only `button0`
    // registers — the element the Slint side forwards.
    val chipLevels = when {
        !widget.enabled -> FabElevationLevels(0.dp, 0.dp, 0.dp, 0.dp)
        widget.kind.startsWith("elevated-") -> FabElevationLevels(1.dp, 1.dp, 1.dp, 3.dp)
        widget.kind == "filter-chip" -> FabElevationLevels(0.dp, 0.dp, 0.dp, 1.dp)
        else -> FabElevationLevels(0.dp, 0.dp, 0.dp, 0.dp)
    }
    val chipShadow = fabShadowElevationProbe(interactionSource, chipLevels)
    val chipPressed by interactionSource.collectIsPressedAsState()
    val motionScheme = androidx.compose.material3.MaterialTheme.motionScheme
    // Expressive `shapes:` morph: ChipsTokens UnselectedShape CornerMedium
    // (8), SelectedShape CornerFull (half the 32dp height), PressedShape
    // CornerSmall (4). The static `shape:` overloads' ContainerShape is
    // CornerSmall on every chip kind.
    val cornerTarget =
        if (expressive && chipPressed) {
            4.dp
        } else if (expressive && selected) {
            16.dp
        } else if (expressive) {
            8.dp
        } else {
            4.dp
        }
    val cornerAnim by animateDpAsState(
        cornerTarget,
        motionScheme.fastSpatialSpec(),
        label = "chip_corner_top_left",
    )
    if (elementId == "button0") {
        tracer.propGetters["shadow_elevation"] = { chipShadow.value.toDouble() }
        tracer.propGetters["corner_top_left"] = { cornerAnim.value.toDouble() }
    }

    when (widget.kind) {
        "assist-chip" -> AssistChip(
            onClick = {},
            label = label,
            modifier = modifier,
            enabled = widget.enabled,
            leadingIcon = leadingIcon,
            trailingIcon = trailingIcon,
            interactionSource = interactionSource,
        )
        "elevated-assist-chip" -> ElevatedAssistChip(
            onClick = {},
            label = label,
            modifier = modifier,
            enabled = widget.enabled,
            leadingIcon = leadingIcon,
            trailingIcon = trailingIcon,
            interactionSource = interactionSource,
        )
        "filter-chip" -> if (expressive) {
            FilterChip(
                selected = selected,
                onClick = {},
                label = label,
                shapes = FilterChipDefaults.shapes(),
                modifier = modifier,
                enabled = widget.enabled,
                leadingIcon = leadingIcon,
                trailingIcon = trailingIcon,
                interactionSource = interactionSource,
            )
        } else {
            FilterChip(
                selected = selected,
                onClick = {},
                label = label,
                modifier = modifier,
                enabled = widget.enabled,
                leadingIcon = leadingIcon,
                trailingIcon = trailingIcon,
                interactionSource = interactionSource,
            )
        }
        "elevated-filter-chip" -> if (expressive) {
            ElevatedFilterChip(
                selected = selected,
                onClick = {},
                label = label,
                shapes = FilterChipDefaults.shapes(),
                modifier = modifier,
                enabled = widget.enabled,
                leadingIcon = leadingIcon,
                trailingIcon = trailingIcon,
                interactionSource = interactionSource,
            )
        } else {
            ElevatedFilterChip(
                selected = selected,
                onClick = {},
                label = label,
                modifier = modifier,
                enabled = widget.enabled,
                leadingIcon = leadingIcon,
                trailingIcon = trailingIcon,
                interactionSource = interactionSource,
            )
        }
        "input-chip" -> if (expressive) {
            InputChip(
                selected = selected,
                onClick = {},
                label = label,
                shapes = InputChipDefaults.shapes(),
                modifier = modifier,
                enabled = widget.enabled,
                leadingIcon = leadingIcon,
                avatar = avatar,
                trailingIcon = trailingIcon,
                interactionSource = interactionSource,
            )
        } else {
            InputChip(
                selected = selected,
                onClick = {},
                label = label,
                modifier = modifier,
                enabled = widget.enabled,
                leadingIcon = leadingIcon,
                avatar = avatar,
                trailingIcon = trailingIcon,
                interactionSource = interactionSource,
            )
        }
        "suggestion-chip" -> SuggestionChip(
            onClick = {},
            label = label,
            modifier = modifier,
            enabled = widget.enabled,
            icon = leadingIcon,
            interactionSource = interactionSource,
        )
        "elevated-suggestion-chip" -> ElevatedSuggestionChip(
            onClick = {},
            label = label,
            modifier = modifier,
            enabled = widget.enabled,
            icon = leadingIcon,
            interactionSource = interactionSource,
        )
        else -> error("unknown chip kind ${widget.kind}")
    }
}
