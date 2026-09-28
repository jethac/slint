// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

package org.slint.material.parity

import androidx.compose.ui.geometry.Rect
import org.json.JSONArray
import org.json.JSONObject

/** What the Compose side records per rendered frame: property values plus the
 * root-relative bounds of every element the scene's `trace_elements` names —
 * the same shape the Slint harness's `trace.json` comparison reads. */
class Tracer {
    /** Element id → bounds (px, root-relative) as reported by layout. */
    val elementBounds = mutableMapOf<String, Rect>()

    /** Element id → opacity (declared by the scene catalog). */
    val elementOpacity = mutableMapOf<String, Float>()

    /** Trace-prop name → current value, filled by the composable each frame. */
    val propGetters = mutableMapOf<String, () -> Any>()

    fun frame(tMs: Long, props: List<String>, elements: List<String>, density: Float): JSONObject {
        val propsJson = JSONObject()
        for (name in props) {
            val v = propGetters[name]?.invoke() ?: continue
            propsJson.put(name, when (v) {
                is Number -> v.toDouble()
                is Boolean -> v
                else -> v.toString()
            })
        }
        val elementsJson = JSONObject()
        for (id in elements) {
            val b = elementBounds[id] ?: continue
            elementsJson.put(
                id,
                JSONObject()
                    .put("x", b.left / density)
                    .put("y", b.top / density)
                    .put("w", b.width / density)
                    .put("h", b.height / density)
                    .put("opacity", elementOpacity[id]?.toDouble() ?: 1.0),
            )
        }
        return JSONObject().put("t_ms", tMs).put("props", propsJson).put("elements", elementsJson)
    }
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
