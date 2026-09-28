// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

package org.slint.material.parity

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color as AndroidColor
import android.graphics.Paint
import android.graphics.Rect as AndroidRect
import android.graphics.Typeface
import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performMouseInput
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.Density
import java.io.File
import java.io.FileOutputStream
import org.json.JSONObject
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** Renders every parity scene at each of its densities on Jetpack Compose
 * material3 and writes the reference PNGs, text masks, and `trace.json` the
 * Slint screenshot driver compares against. Driven by the mocked compose
 * clock, so frames are deterministic.
 *
 * Renders land under `build/parity-out/`; `-Dparity.record` (the Gradle
 * `parity.record` property) writes them into `references/` to regenerate the
 * committed references. */
@RunWith(RobolectricTestRunner::class)
class ParityTest {

    @get:Rule val rule = createAndroidComposeRule<ComponentActivity>()

    @Test
    fun renderAllScenes() {
        val font = loadRoboto()
        for (scene in Scene.loadAll()) {
            for (density in scene.densities) {
                renderScene(scene, density, font)
            }
        }
    }

    private fun renderScene(scene: Scene, density: Int, font: FontFamily) {
        val tracer = Tracer()
        val motion = scene.parity == "motion"
        rule.mainClock.autoAdvance = false
        rule.setContent {
            CompositionLocalProvider(LocalDensity provides Density(density.toFloat(), 1f)) {
                SceneContent(scene, font, tracer)
            }
        }
        rule.waitForIdle()

        applyActions(scene, density)

        // Static cases settle out entry animations and ripples before the
        // shot, mirroring `STATIC_SETTLE_MS` in tests/screenshots/parity.rs.
        val times = if (motion) scene.times else listOf(STATIC_SETTLE_MS)
        val frames = mutableListOf<JSONObject>()
        var at = 0L
        for (t in times) {
            rule.mainClock.advanceTimeBy(t - at)
            at = t
            rule.waitForIdle()
            val tag = if (motion) "${t}ms" else "settled"
            val image = rule.onNodeWithTag("scene-root").captureToImage().asAndroidBitmap()
            writePng(outDir(scene, density).resolve("frame_$tag.png"), image)
            writeMask(outDir(scene, density).resolve("mask_$tag.png"), image.width, image.height, tracer)
            frames += tracer.frame(t, scene.traceProps, scene.traceElements, density.toFloat())
        }

        if (motion) {
            val trace = JSONObject()
                .put("times_ms", scene.times)
                .put("frames", org.json.JSONArray(frames))
                .put("settle_ms", settleTimeMs(frames) ?: JSONObject.NULL)
            outDir(scene, density).resolve("trace.json").writeText(trace.toString(2))
        }
    }

    /** `//ACTION=` pointer steps, scaled by density, in order. */
    private fun applyActions(scene: Scene, density: Int) {
        val root = rule.onNodeWithTag("scene-root")
        for (a in scene.actions) {
            val at = Offset(a.x * density, a.y * density)
            when (a.kind) {
                "move" -> root.performMouseInput { enter(at) }
                "press" -> root.performTouchInput { down(at) }
                "release" -> root.performTouchInput { up() }
                else -> error("unknown action ${a.kind}")
            }
        }
        rule.waitForIdle()
    }

    /** The text mask: black everywhere except the bounds of the scene's
     * `text:*` nodes, in white. Emitted only for frames that contain text. */
    private fun writeMask(path: File, width: Int, height: Int, tracer: Tracer) {
        val texts = tracer.elementBounds.filterKeys { it.startsWith("text:") }
        if (texts.isEmpty()) return
        val bmp = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bmp)
        canvas.drawColor(AndroidColor.BLACK)
        val paint = Paint().apply { color = AndroidColor.WHITE }
        for ((_, r) in texts) {
            canvas.drawRect(
                AndroidRect(
                    r.left.toInt(),
                    r.top.toInt(),
                    r.right.toInt(),
                    r.bottom.toInt(),
                ),
                paint,
            )
        }
        writePng(path, bmp)
    }

    private fun writePng(path: File, bitmap: Bitmap) {
        path.parentFile?.mkdirs()
        FileOutputStream(path).use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
    }

    private fun outDir(scene: Scene, density: Int): File =
        File(
            File(System.getProperty("parity.out.dir", "build/parity-out")),
            "${scene.caseRel}/d$density",
        )

    /** Roboto, loaded from the same file the Slint driver registers. */
    private fun loadRoboto(): FontFamily {
        val stream = javaClass.classLoader.getResourceAsStream("fonts/roboto.ttf")
            ?: error("fonts/roboto.ttf missing from test resources")
        val file = File.createTempFile("roboto", ".ttf")
        file.deleteOnExit()
        stream.use { input -> file.outputStream().use(input::copyTo) }
        val typeface = Typeface.createFromFile(file)
        return FontFamily(
            androidx.compose.ui.text.font.Typeface(typeface),
        )
    }

    companion object {
        private const val STATIC_SETTLE_MS = 2000L
    }
}
