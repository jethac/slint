// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

package org.slint.material.parity

import androidx.compose.ui.geometry.Rect
import org.json.JSONObject

/** Layout bounds plus text-layout metrics of one tracked node, captured at a
 * frame boundary. `baseline` is the distance from the top edge to the first
 * text baseline, `lines` the laid-out line count, `fracW` the unhinted
 * `Paint.measureText` width of the same string — the font's own advance
 * metrics. Layoutlib's `Text` measures hinted integer advances, so `w` runs
 * roughly a pixel wider per glyph than `fracW`; Slint's fractional text
 * layout matches `fracW`, and the comparator validates each side against it.
 * `chars` is the laid-out string length, used to bound the expected drift. */
data class TextMetric(
    val x: Double,
    val y: Double,
    val w: Double,
    val h: Double,
    val baseline: Double,
    val lines: Int,
    val fracW: Double,
    val chars: Int,
) {
    fun toJson(): JSONObject = JSONObject()
        .put("x", x)
        .put("y", y)
        .put("w", w)
        .put("h", h)
        .put("baseline", baseline)
        .put("lines", lines)
        .put("frac_w", fracW)
        .put("chars", chars)
}

/** What the Compose side records per rendered frame: property values, the
 * root-relative bounds of every element the scene's `trace_elements` names,
 * and metrics of every `text:` node — the same shape the Slint harness's
 * `trace.json` comparison reads. Frames are recorded at every Composable
 * frame tick (the Paparazzi-driven clock), then downsampled to the scene's
 * `times`. */
class Tracer(
    /** Font-weight (400/500/600/700) → static Roboto instance file, for
     * [fracWidth]. */
    private val fontFiles: Map<Int, java.io.File> = emptyMap(),
) {
    /** Element id → bounds (px, root-relative), refreshed by layout each frame. */
    val elementBounds = mutableMapOf<String, Rect>()

    /** Element id → opacity (declared by the scene catalog). */
    val elementOpacity = mutableMapOf<String, Float>()

    /** Trace-prop name → current value, read by [recordFrame]. */
    val propGetters = mutableMapOf<String, () -> Any>()

    /** `text:<id>` → layout metrics, refreshed by `onTextLayout`. */
    val textMetrics = mutableMapOf<String, TextMetric>()

    /** Every recorded frame, in frame order. */
    val frames = mutableListOf<JSONObject>()

    /** Element ids whose first layout pass has run — `recordFrame` at t=0
     * happens before them, so their first observed bounds also fill any
     * already-recorded frame that lacks the id (the element sits at its
     * initial position until a gesture fires). */
    private val layoutSeen = mutableSetOf<String>()
    private var density = 1f

    fun noteDensity(d: Float) {
        density = d
    }

    /** Unhinted width of `text` rendered with `style` — the font's own
     * advance metrics, independent of layoutlib's hinted layout.
     * `fracW` is in dp like every metric the comparator reads. */
    fun fracWidth(
        text: String,
        style: androidx.compose.ui.text.TextStyle,
        density: androidx.compose.ui.unit.Density,
    ): Double {
        val weight = style.fontWeight?.weight ?: 500
        val file = fontFiles[weight] ?: fontFiles.entries.minByOrNull { kotlin.math.abs(it.key - weight) }?.value
            ?: return Double.NaN
        if (!style.fontSize.isSp) return Double.NaN
        with(density) {
            val sizePx = style.fontSize.toPx()
            val spacingEm = when {
                style.letterSpacing.isEm -> style.letterSpacing.value
                style.letterSpacing.isSp -> style.letterSpacing.toPx() / sizePx
                else -> 0f
            }
            val paint = android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG).apply {
                typeface = android.graphics.Typeface.createFromFile(file)
                textSize = sizePx
                hinting = android.graphics.Paint.HINTING_OFF
                letterSpacing = spacingEm
            }
            return paint.measureText(text) / density.density.toDouble()
        }
    }

    /** Called by `track` once the first layout of `id` has run: earlier
     * frames (frame 0) were recorded before it, so fill the initial bounds. */
    fun backfill(id: String, b: Rect) {
        if (!layoutSeen.add(id)) return
        for (f in frames) {
            if (f.getJSONObject("elements").has(id)) break
            f.getJSONObject("elements").put(id, boundsJson(b, density))
        }
    }

    /** Snapshot the current values as the frame at `tMs` (the Paparazzi
     * frame-clock time in milliseconds). */
    fun recordFrame(tMs: Long, props: List<String>, elements: List<String>, density: Float) {
        val propsJson = JSONObject()
        for (name in props) {
            val v = propGetters[name]?.invoke() ?: continue
            propsJson.put(
                name,
                when (v) {
                    is Number -> v.toDouble()
                    is Boolean -> v
                    else -> v.toString()
                },
            )
        }
        val elementsJson = JSONObject()
        for (id in elements) {
            val b = elementBounds[id] ?: continue
            elementsJson.put(id, boundsJson(b, density).put("opacity", elementOpacity[id]?.toDouble() ?: 1.0))
        }
        val textJson = JSONObject()
        for ((id, m) in textMetrics.toSortedMap()) {
            textJson.put(id, m.toJson())
        }
        frames += JSONObject()
            .put("t_ms", tMs)
            .put("props", propsJson)
            .put("elements", elementsJson)
            .put("text", textJson)
    }

    /** The recorded `trace.json`: every recorded frame (the Slint comparator
     * needs them to tolerate the two engines' frame-clock phase offset), plus
     * `settle_ms`. */
    fun traceJson(times: List<Long>): JSONObject {
        return JSONObject()
            .put("times_ms", org.json.JSONArray(times))
            .put("frames", org.json.JSONArray(frames))
            .put("settle_ms", settleTimeMs(frames) ?: JSONObject.NULL)
    }

    private fun boundsJson(b: Rect, density: Float): JSONObject = JSONObject()
        .put("x", b.left / density)
        .put("y", b.top / density)
        .put("w", b.width / density)
        .put("h", b.height / density)
}

/** `settle_ms`: the last timestamp where a numeric trace value still differs
 * from its final value by more than `eps`, matching `settle_time_ms` in
 * `tests/screenshots/parity.rs`. `null` when the trace never moved. */
fun settleTimeMs(frames: List<JSONObject>, eps: Double = 0.05): Long? {
    val last = frames.lastOrNull() ?: return null
    var settle: Long = 0
    var moved = false
    val finalProps = last.getJSONObject("props")
    val finalElements = last.getJSONObject("elements")
    for (f in frames) {
        val t = f.getLong("t_ms")
        for (name in finalProps.keys()) {
            val a = f.getJSONObject("props").optDouble(name, Double.NaN)
            val b = finalProps.optDouble(name, Double.NaN)
            if (!a.isNaN() && !b.isNaN() && kotlin.math.abs(a - b) > eps) {
                settle = t; moved = true
            }
        }
        for (id in finalElements.keys()) {
            val a = f.getJSONObject("elements").optJSONObject(id) ?: continue
            val b = finalElements.getJSONObject(id)
            for (k in listOf("x", "y", "w", "h", "opacity")) {
                if (kotlin.math.abs(a.optDouble(k, 0.0) - b.optDouble(k, 0.0)) > eps) {
                    settle = t; moved = true
                }
            }
        }
    }
    return if (moved) settle else null
}
