/*
 * Copyright © SixtyFPS GmbH <info@slint.dev>
 * SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
 *
 * Golden-vector generator for the Slint shapes parity tests.
 *
 * Compiles the pinned androidx.graphics.shapes sources (vendored under
 * src/androidx/graphics/shapes/) plus MaterialShapes.kt at
 * 23327507f7fc7d5b19d65fec4b090f60c970079b and emits JSON where every f32 is
 * encoded as its IEEE-754 bit pattern (u32) so comparisons are bit-exact.
 */

@file:OptIn(androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)

import androidx.compose.material3.MaterialShapes
import androidx.graphics.shapes.CornerRounding
import androidx.graphics.shapes.Cubic
import androidx.graphics.shapes.Feature
import androidx.graphics.shapes.Morph
import androidx.graphics.shapes.RoundedPolygon
import androidx.graphics.shapes.circle
import androidx.graphics.shapes.pill
import androidx.graphics.shapes.pillStar
import androidx.graphics.shapes.rectangle
import androidx.graphics.shapes.star
import java.io.File

private fun f(x: Float): String = java.lang.Integer.toUnsignedString(x.toRawBits())

private fun cubicsJson(cubics: List<Cubic>): String {
    val sb = StringBuilder("[")
    cubics.forEachIndexed { i, c ->
        if (i > 0) sb.append(',')
        sb.append('[')
        for (j in 0 until 8) {
            if (j > 0) sb.append(',')
            sb.append(f(c.points[j]))
        }
        sb.append(']')
    }
    return sb.append(']').toString()
}

private fun featuresJson(polygon: RoundedPolygon): String {
    val sb = StringBuilder("[")
    polygon.features.forEachIndexed { i, feature ->
        if (i > 0) sb.append(',')
        when (feature) {
            is Feature.Corner ->
                sb.append(
                    "{\"kind\":\"corner\",\"convex\":${feature.convex}," +
                        "\"count\":${feature.cubics.size}}"
                )
            is Feature.Edge ->
                sb.append("{\"kind\":\"edge\",\"count\":${feature.cubics.size}}")
        }
    }
    return sb.append(']').toString()
}

private fun polygonJson(polygon: RoundedPolygon): String {
    val bounds = polygon.calculateBounds()
    val maxBounds = polygon.calculateMaxBounds()
    return "{\"cubics\":${cubicsJson(polygon.cubics)}," +
        "\"features\":${featuresJson(polygon)}," +
        "\"center\":[${f(polygon.centerX)},${f(polygon.centerY)}]," +
        "\"bounds\":[${bounds.joinToString(",") { f(it) }}]," +
        "\"max_bounds\":[${maxBounds.joinToString(",") { f(it) }}]}"
}

private val MATERIAL_SHAPES: List<Pair<String, () -> RoundedPolygon>> =
    listOf(
        "circle" to { MaterialShapes.Circle },
        "square" to { MaterialShapes.Square },
        "slanted" to { MaterialShapes.Slanted },
        "arch" to { MaterialShapes.Arch },
        "fan" to { MaterialShapes.Fan },
        "arrow" to { MaterialShapes.Arrow },
        "semi_circle" to { MaterialShapes.SemiCircle },
        "oval" to { MaterialShapes.Oval },
        "pill" to { MaterialShapes.Pill },
        "triangle" to { MaterialShapes.Triangle },
        "diamond" to { MaterialShapes.Diamond },
        "clam_shell" to { MaterialShapes.ClamShell },
        "pentagon" to { MaterialShapes.Pentagon },
        "gem" to { MaterialShapes.Gem },
        "sunny" to { MaterialShapes.Sunny },
        "very_sunny" to { MaterialShapes.VerySunny },
        "cookie_4_sided" to { MaterialShapes.Cookie4Sided },
        "cookie_6_sided" to { MaterialShapes.Cookie6Sided },
        "cookie_7_sided" to { MaterialShapes.Cookie7Sided },
        "cookie_9_sided" to { MaterialShapes.Cookie9Sided },
        "cookie_12_sided" to { MaterialShapes.Cookie12Sided },
        "ghostish" to { MaterialShapes.Ghostish },
        "clover_4_leaf" to { MaterialShapes.Clover4Leaf },
        "clover_8_leaf" to { MaterialShapes.Clover8Leaf },
        "burst" to { MaterialShapes.Burst },
        "soft_burst" to { MaterialShapes.SoftBurst },
        "boom" to { MaterialShapes.Boom },
        "soft_boom" to { MaterialShapes.SoftBoom },
        "flower" to { MaterialShapes.Flower },
        "puffy" to { MaterialShapes.Puffy },
        "puffy_diamond" to { MaterialShapes.PuffyDiamond },
        "pixel_circle" to { MaterialShapes.PixelCircle },
        "pixel_triangle" to { MaterialShapes.PixelTriangle },
        "bun" to { MaterialShapes.Bun },
        "heart" to { MaterialShapes.Heart },
    )

private val PRIMITIVE_SHAPES: List<Pair<String, () -> RoundedPolygon>> =
    listOf(
        "circle_default" to { RoundedPolygon.circle() },
        "circle_5" to { RoundedPolygon.circle(numVertices = 5) },
        "circle_3_smoothing" to
            { RoundedPolygon.circle(numVertices = 3, radius = 1.5f, centerX = 0.4f, centerY = 0.6f) },
        "rectangle_default" to { RoundedPolygon.rectangle() },
        "rectangle_2x1_rounded" to
            { RoundedPolygon.rectangle(width = 2f, height = 1f, rounding = CornerRounding(0.15f, 0.4f)) },
        "star_default" to { RoundedPolygon.star(numVerticesPerRadius = 5) },
        "star_7" to
            {
                RoundedPolygon.star(
                    numVerticesPerRadius = 7,
                    radius = 2f,
                    innerRadius = 0.9f,
                    rounding = CornerRounding(0.1f, 0.5f),
                    innerRounding = CornerRounding(0.05f),
                )
            },
        "pill_2x1" to { RoundedPolygon.pill(width = 2f, height = 1f) },
        "pill_1x3_smoothing" to { RoundedPolygon.pill(width = 1f, height = 3f, smoothing = 0.8f) },
        "pill_star_8" to { RoundedPolygon.pillStar(width = 2f, height = 1f, numVerticesPerRadius = 8) },
        "pill_star_5_inner03" to
            {
                RoundedPolygon.pillStar(
                    width = 1f,
                    height = 2f,
                    numVerticesPerRadius = 5,
                    innerRadiusRatio = 0.3f,
                    rounding = CornerRounding(0.05f, 0.7f),
                    vertexSpacing = 0.8f,
                    startLocation = 0.25f,
                )
            },
    )

private data class MorphSpec(val name: String, val from: RoundedPolygon, val to: RoundedPolygon)

fun main(args: Array<String>) {
    val outDir = File(if (args.isNotEmpty()) args[0] else "golden")
    outDir.mkdirs()

    // --- material_shapes.json: the 35 MaterialShapes entries -------------------
    val shapesSb = StringBuilder()
    MATERIAL_SHAPES.forEachIndexed { i, (name, build) ->
        if (i > 0) shapesSb.append(",\n")
        val polygon = build()
        shapesSb.append("\"$name\":${polygonJson(polygon)}")
    }
    PRIMITIVE_SHAPES.forEach { (name, build) ->
        shapesSb.append(",\n\"$name\":${polygonJson(build())}")
    }
    File(outDir, "material_shapes.json")
        .writeText(
            "{\"pin\":\"23327507f7fc7d5b19d65fec4b090f60c970079b\",\n" +
                "\"shapes\":{\n$shapesSb\n}}\n"
        )

    // --- morphs.json: sampled morphs for fixed pairs ---------------------------
    val shapesByName = MATERIAL_SHAPES.associate { (n, b) -> n to b() }
    val specs =
        listOf(
            MorphSpec("cookie_9_sided->circle", shapesByName.getValue("cookie_9_sided"), shapesByName.getValue("circle")),
            MorphSpec("circle->cookie_9_sided", shapesByName.getValue("circle"), shapesByName.getValue("cookie_9_sided")),
            MorphSpec("cookie_7_sided->clover_8_leaf", shapesByName.getValue("cookie_7_sided"), shapesByName.getValue("clover_8_leaf")),
            MorphSpec("triangle->square", shapesByName.getValue("triangle"), shapesByName.getValue("square")),
            MorphSpec("burst->oval", shapesByName.getValue("burst"), shapesByName.getValue("oval")),
            MorphSpec("heart->heart", shapesByName.getValue("heart"), shapesByName.getValue("heart")),
            MorphSpec("pill->pill_star_8", RoundedPolygon.pill(width = 2f, height = 1f), RoundedPolygon.pillStar(width = 2f, height = 1f, numVerticesPerRadius = 8)),
        )
    val progress = listOf(0f, 0.25f, 0.5f, 0.75f, 1f)
    val morphsSb = StringBuilder()
    specs.forEachIndexed { i, spec ->
        if (i > 0) morphsSb.append(",\n")
        val morph = Morph(spec.from, spec.to)
        morphsSb.append("\"${spec.name}\":{")
        progress.forEachIndexed { j, t ->
            if (j > 0) morphsSb.append(',')
            morphsSb.append("\"$t\":${cubicsJson(morph.asCubics(t))}")
        }
        morphsSb.append('}')
    }
    File(outDir, "morphs.json")
        .writeText(
            "{\"pin\":\"23327507f7fc7d5b19d65fec4b090f60c970079b\",\n" +
                "\"morphs\":{\n$morphsSb\n}}\n"
        )

    println("Wrote ${outDir}/material_shapes.json and ${outDir}/morphs.json")
}
