// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

package org.slint.material.parity

import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import com.android.resources.Density
import java.io.File
import app.cash.paparazzi.TestName
import org.junit.Test

/** Renders every parity scene on Jetpack Compose material3 through Paparazzi
 * (layoutlib — the same rasterizer Android Studio previews use) and writes the
 * reference PNGs, text masks, and `trace.json` the Slint screenshot driver
 * compares against.
 *
 * Paparazzi advances the Compose frame clock in exact `1/fps` steps; at
 * `fps = 1000` frame `i` is rendered at `t = i` ms, so scene timestamps map to
 * frame indexes exactly. Renders land under `build/parity-out/`; the Gradle
 * property `-Pparity.record` writes them into `references/` to regenerate the
 * committed references. */
class RenderTest {

    private val frames = FrameSink()

    @Test
    fun renderAllScenes() {
        // -Dparity.scene=<name> restricts the run to one scene (debugging).
        val only = System.getProperty("parity.scene")
        val testName = TestName(
            RenderTest::class.java.packageName,
            RenderTest::class.java.name,
            "renderAllScenes",
        )
        for (scene in Scene.loadAll()) {
            if (only != null && scene.name != only) continue
            // The generator resolves the scheme with Slint's own
            // material-color-utils port; the Compose side re-derives it with
            // the Kotlin MCU port. If they disagree a port has drifted —
            // fail here, not in a pixel diff.
            assertSchemeMatches(scene)
            // A fresh Paparazzi per scene: one layoutlib render session per
            // gif() leaks ImageReader buffers, and `teardown()` is the only
            // point they are released — long runs SIGABRT without it.
            val paparazzi = Paparazzi(
                deviceConfig = DeviceConfig.NEXUS_5,
                theme = "android:Theme.Material.Light.NoActionBar",
                snapshotHandler = frames,
                appCompatEnabled = false,
                // Render at the configured pixel size — the default scales
                // renders down when the surface exceeds layoutlib's cap,
                // breaking the 1:1 physical-pixel compare at 2x.
                useDeviceResolution = true,
            )
            try {
                paparazzi.setup(testName)
                for (density in scene.densities) {
                    renderScene(paparazzi, scene, density)
                }
            } finally {
                paparazzi.teardown()
            }
        }
    }

    private fun renderScene(paparazzi: Paparazzi, scene: Scene, density: Int) {
        val (w, h) = scene.sizeDp
        paparazzi.unsafeUpdateConfig(deviceConfig = deviceFor(w, h, density))
        val outDir = outDir(scene, density)
        outDir.deleteRecursively()
        val motion = scene.parity == "motion"
        val tracer = Tracer(robotoFontFiles())
        tracer.noteDensity(density.toFloat())

        val host = ComposeView(paparazzi.context)
        host.setContent { SceneContent(scene, robotoFamily(), tracer) }

        // fps = 1000 makes frame indexes equal milliseconds; static scenes
        // step at 50 ms and keep only the settled last frame. Static scenes
        // start at 1 ms — a held `pressed` ripple requesting a frame at
        // t = 0 aborts layoutlib natively ("0 isn't a real frame time").
        val wanted = if (motion) scene.times.toSet() else emptySet()
        val lastMs = if (motion) scene.times.max() else STATIC_SETTLE_MS + 1
        val startMs = if (motion) 0L else 1L
        val fps = if (motion) 1000 else 20

        frames.setSink(java.util.function.BiConsumer { index, image ->
            if (motion && index.toLong() in wanted) {
                FrameSink.writeFrame(outDir.resolve("frame_${index}ms.png"), image)
            }
        })
        frames.setOnFramesDone(java.lang.Runnable {
            if (!motion) {
                frames.writeLast(outDir.resolve("frame_settled.png"))
            }
        })

        paparazzi.gif(host, scene.name, startMs, lastMs, fps)
        frames.setSink(null)
        frames.setOnFramesDone(null)

        writeMasks(outDir, motion, wanted, tracer, density.toFloat(), w * density, h * density)
        val times = if (motion) scene.times else listOf(STATIC_SETTLE_MS)
        outDir.resolve("trace.json").writeText(tracer.traceJson(times).toString(2))
    }

    /** One mask per frame: white inside the text nodes' bounds, black
     * elsewhere. For motion scenes every wanted timestamp; for static scenes
     * the last frame that recorded text metrics. */
    private fun writeMasks(
        outDir: File,
        motion: Boolean,
        wanted: Set<Long>,
        tracer: Tracer,
        density: Float,
        widthPx: Int,
        heightPx: Int,
    ) {
        val lastTextFrame = tracer.frames.lastOrNull { it.getJSONObject("text").length() > 0 }
        for (frame in tracer.frames) {
            val t = frame.getLong("t_ms")
            if (motion && t !in wanted) continue
            if (!motion && frame !== lastTextFrame) continue
            val text = frame.getJSONObject("text")
            if (text.length() == 0) continue
            val name = if (motion) "mask_${t}ms.png" else "mask_settled.png"
            Png.writeMask(outDir.resolve(name), text, density, widthPx, heightPx)
        }
    }

    /** Weight → extracted static Roboto instance file
     * (`src/test/resources/fonts/roboto-*.ttf`), baked from
     * `tests/screenshots/fonts/Roboto-VariableFont.ttf` with
     * `instantiateVariableFont(wght=N)` — the generator enforces they're
     * byte-identical. Layoutlib ignores `FontVariation` on file-loaded
     * variable fonts (it emboldens instead of resolving the instance), so
     * each weight gets its own file. */
    private fun robotoFontFiles(): Map<Int, File> =
        listOf(400, 500, 600, 700).associateWith { weight ->
            val tmp = File.createTempFile("parity-roboto-$weight", ".ttf")
                .apply { deleteOnExit() }
            javaClass.classLoader!!.getResourceAsStream("fonts/roboto-$weight.ttf")!!.use {
                tmp.writeBytes(it.readBytes())
            }
            tmp
        }

    private fun robotoFamily(): FontFamily {
        val files = robotoFontFiles()
        return FontFamily(
            Font(files.getValue(400), weight = FontWeight.Normal),
            Font(files.getValue(500), weight = FontWeight.Medium),
            Font(files.getValue(600), weight = FontWeight.SemiBold),
            Font(files.getValue(700), weight = FontWeight.Bold),
        )
    }

    private fun outDir(scene: Scene, density: Int): File =
        File(
            File(System.getProperty("parity.out.dir", "build/parity-out")),
            "${scene.caseRel}/d$density",
        )

    private fun deviceFor(w: Int, h: Int, density: Int): DeviceConfig {
        val d = when (density) {
            1 -> Density.MEDIUM
            2 -> Density.XHIGH
            else -> error("no Density constant for ${density}x")
        }
        return DeviceConfig(
            screenWidth = w * density,
            screenHeight = h * density,
            xdpi = 160 * density,
            ydpi = 160 * density,
            density = d,
            // Without LANDSCAPE layoutlib swaps the screen dims for any
            // scene wider than tall (portrait is the default).
            orientation = com.android.resources.ScreenOrientation.LANDSCAPE,
            softButtons = false,
        )
    }

    companion object {
        private const val STATIC_SETTLE_MS = 2000L
    }
}
