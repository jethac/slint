// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

package org.slint.material.parity

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Button
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.Typography
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlin.math.PI

/** Every traced element reports its bounds here; ids prefixed `text:` are the
 * text nodes the mask PNG is built from. */
fun Modifier.track(tracer: Tracer, id: String): Modifier =
    onGloballyPositioned { coords: LayoutCoordinates ->
        tracer.elementBounds[id] = coords.boundsInRoot()
    }.testTag(id)

/** The scene's color scheme: `lightColorScheme` built from the generator's
 * resolved ARGB roles. */
fun colorScheme(scheme: Map<String, Long>): ColorScheme =
    lightColorScheme(
        primary = color(scheme, "primary"),
        onPrimary = color(scheme, "onPrimary"),
        primaryContainer = color(scheme, "primaryContainer"),
        onPrimaryContainer = color(scheme, "onPrimaryContainer"),
        inversePrimary = color(scheme, "inversePrimary"),
        secondary = color(scheme, "secondary"),
        onSecondary = color(scheme, "onSecondary"),
        secondaryContainer = color(scheme, "secondaryContainer"),
        onSecondaryContainer = color(scheme, "onSecondaryContainer"),
        tertiary = color(scheme, "tertiary"),
        onTertiary = color(scheme, "onTertiary"),
        tertiaryContainer = color(scheme, "tertiaryContainer"),
        onTertiaryContainer = color(scheme, "onTertiaryContainer"),
        background = color(scheme, "background"),
        onBackground = color(scheme, "onBackground"),
        surface = color(scheme, "surface"),
        onSurface = color(scheme, "onSurface"),
        surfaceVariant = color(scheme, "surfaceVariant"),
        surfaceTint = color(scheme, "surfaceTint"),
        inverseSurface = color(scheme, "inverseSurface"),
        inverseOnSurface = color(scheme, "inverseOnSurface"),
        error = color(scheme, "error"),
        onError = color(scheme, "onError"),
        errorContainer = color(scheme, "errorContainer"),
        onErrorContainer = color(scheme, "onErrorContainer"),
        outline = color(scheme, "outline"),
        outlineVariant = color(scheme, "outlineVariant"),
        scrim = color(scheme, "scrim"),
        surfaceBright = color(scheme, "surfaceBright"),
        surfaceContainer = color(scheme, "surfaceContainer"),
        surfaceContainerHigh = color(scheme, "surfaceContainerHigh"),
        surfaceContainerHighest = color(scheme, "surfaceContainerHighest"),
        surfaceContainerLow = color(scheme, "surfaceContainerLow"),
        surfaceContainerLowest = color(scheme, "surfaceContainerLowest"),
        surfaceDim = color(scheme, "surfaceDim"),
        primaryFixed = color(scheme, "primaryFixed"),
        primaryFixedDim = color(scheme, "primaryFixedDim"),
        onPrimaryFixed = color(scheme, "onPrimaryFixed"),
        onPrimaryFixedVariant = color(scheme, "onPrimaryFixedVariant"),
        secondaryFixed = color(scheme, "secondaryFixed"),
        secondaryFixedDim = color(scheme, "secondaryFixedDim"),
        onSecondaryFixed = color(scheme, "onSecondaryFixed"),
        onSecondaryFixedVariant = color(scheme, "onSecondaryFixedVariant"),
        tertiaryFixed = color(scheme, "tertiaryFixed"),
        tertiaryFixedDim = color(scheme, "tertiaryFixedDim"),
        onTertiaryFixed = color(scheme, "onTertiaryFixed"),
        onTertiaryFixedVariant = color(scheme, "onTertiaryFixedVariant"),
    )

private fun color(scheme: Map<String, Long>, role: String): Color =
    Color((scheme[role] ?: error("scheme is missing role $role")).toULong())

/** Material typography with the scene's font family on every level. */
fun typography(font: FontFamily): Typography =
    Typography(
        displayLarge = TextStyle(fontFamily = font),
        displayMedium = TextStyle(fontFamily = font),
        displaySmall = TextStyle(fontFamily = font),
        headlineLarge = TextStyle(fontFamily = font),
        headlineMedium = TextStyle(fontFamily = font),
        headlineSmall = TextStyle(fontFamily = font),
        titleLarge = TextStyle(fontFamily = font),
        titleMedium = TextStyle(fontFamily = font),
        titleSmall = TextStyle(fontFamily = font),
        bodyLarge = TextStyle(fontFamily = font),
        bodyMedium = TextStyle(fontFamily = font),
        bodySmall = TextStyle(fontFamily = font),
        labelLarge = TextStyle(fontFamily = font),
        labelMedium = TextStyle(fontFamily = font),
        labelSmall = TextStyle(fontFamily = font),
    )

/** The composable a scene renders, keyed on its `type`. */
@Composable
fun SceneContent(scene: Scene, font: FontFamily, tracer: Tracer) {
    MaterialTheme(colorScheme = colorScheme(scene.scheme), typography = typography(font)) {
        when (scene.type) {
            "canvas" -> CanvasScene(scene, tracer)
            "spring-motion" -> SpringMotionScene(scene, tracer)
            else -> error("unknown scene type ${scene.type}")
        }
    }
}

@Composable
private fun CanvasScene(scene: Scene, tracer: Tracer) {
    val (w, h) = scene.sizeDp
    val scheme = MaterialTheme.colorScheme
    Box(
        Modifier.testTag("scene-root").size(w.dp, h.dp).background(scheme.background),
    ) {
        scene.widgets.forEachIndexed { i, widget ->
            when (widget.kind) {
                "filled-button" ->
                    Button(
                        onClick = {},
                        enabled = widget.enabled,
                        modifier = Modifier.offset(widget.x.dp, widget.y.dp),
                    ) {
                        Text(widget.text ?: "", modifier = Modifier.track(tracer, "text:$i"))
                    }
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

private fun RoundedCornerShapeOrRect(radius: Dp): Shape =
    if (radius <= 0.dp) RectangleShape else androidx.compose.foundation.shape.RoundedCornerShape(radius)

@Composable
private fun schemeColor(role: String): Color =
    MaterialTheme.colorScheme.let { scheme ->
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
        label = "box-x",
    )

    tracer.propGetters["box-x"] = { x.value.toDouble() }
    tracer.elementOpacity["thumb"] = 1f

    val scheme = MaterialTheme.colorScheme
    Box(
        Modifier.testTag("scene-root").size(w.dp, h.dp).background(scheme.background),
    ) {
        Box(
            Modifier.track(tracer, "thumb")
                .offset(x, y.dp)
                .size(size.dp)
                .clip(RoundedCornerShapeOrRect(radius.dp))
                .background(schemeColor(p.getString("color"))),
        )
        Box(
            Modifier.fillMaxSize().pointerInput(target) {
                detectTapGestures(onPress = { goal = target })
            },
        )
    }
}
