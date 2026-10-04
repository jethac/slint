// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

package org.slint.material.parity

import androidx.compose.ui.geometry.Rect
import org.json.JSONObject

/** Layout bounds plus text-layout metrics of one tracked node, captured at a
 * frame boundary. `baseline` is the distance from the top edge to the first
 * text baseline, `lines` the laid-out line count, `fracW` the real
 * fractional advance of the laid-out text (`TextLayoutResult`'s line
 * right/left edges) — what the font's own advance metrics produce before
 * the layout rounds the node's width to an integer for `w`. `chars` is the
 * laid-out string length, used to bound the expected drift. */
data class TextMetric(
    val x: Double,
    val y: Double,
    val w: Double,
    val h: Double,
    val baseline: Double,
    val lines: Int,
    val fracW: Double,
    /** Unhinted advance of the same text (`Paint` with `HINTING_OFF`) — the
     * same quantity Slint's text layout computes before it ceils to whole
     * pixels, density-independent. */
    val unhintW: Double,
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
        .put("unhint_w", unhintW)
        .put("chars", chars)
}

/** What the Compose side records per rendered frame: property values, the
 * root-relative bounds of every element the scene's `trace_elements` names,
 * and metrics of every `text:` node — the same shape the Slint harness's
 * `trace.json` comparison reads. Frames are recorded at every Composable
 * frame tick (the Paparazzi-driven clock), then downsampled to the scene's
 * `times`. */
class Tracer {
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
            // A getter reports a non-finite number for "not measurable yet"
            // (e.g. `requireOffset` before the first measure); the prop is
            // absent from that frame, and org.json rejects NaN anyway.
            if (v is Number && (v.toDouble().isNaN() || v.toDouble().isInfinite())) continue
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
            // `parent>child`: every tracked id `parent>child<N>`, in numeric
            // order — the Slint harness indexes repeated children the same
            // way. A skipped index is still emitted (an item hidden by the
            // stagger reports zero bounds upstream, matching the Slint side
            // pruning the invisible subtree).
            if (id.contains('>')) {
                val keyed = elementBounds.keys
                    .filter {
                        it.startsWith(id) &&
                            it.removePrefix(id).let { s -> s.isNotEmpty() && s.all(Char::isDigit) }
                    }
                    .sortedBy { it.removePrefix(id).toInt() }
                for (key in keyed) {
                    val b = elementBounds[key] ?: continue
                    elementsJson.put(
                        key,
                        boundsJson(b, density)
                            .put("opacity", elementOpacity[key]?.toDouble() ?: 1.0),
                    )
                }
                continue
            }
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
