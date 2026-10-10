// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import groovy.json.JsonSlurper

plugins {
    // Paparazzi 2.x supports pre-AGP-9 consumers; the compose plugin supplies
    // the Compose compiler for Kotlin Android.
    id("com.android.library") version "8.13.2"
    id("org.jetbrains.kotlin.android") version "2.3.0"
    id("org.jetbrains.kotlin.plugin.compose") version "2.3.0"
    id("app.cash.paparazzi") version "2.0.0-alpha05"
}

// Rendering choice, recorded for #4: Paparazzi (layoutlib — the same
// rasterizer Android Studio previews run) instead of Robolectric.
// Robolectric's native-graphics path can't initialize the font map on many
// hosts (robolectric/robolectric#9039), while Paparazzi runs fully on the
// JVM, drives the Compose frame clock in exact 1/fps steps (frame index ==
// milliseconds at fps = 1000, so scene TIMES map exactly), and its public
// SnapshotHandler hook writes each rendered frame straight into
// `references/`. Interaction states come from the scene definitions
// (MutableInteractionSource emissions at composition) instead of injected
// pointer events.

android {
    namespace = "org.slint.material.parity"
    compileSdk = 36
    defaultConfig {
        minSdk = 26
    }
    buildFeatures {
        compose = true
    }
    testOptions {
        unitTests {
            // Scenes and fonts live under src/test/resources and must reach
            // the unit test classpath.
            isIncludeAndroidResources = true
            isReturnDefaultValues = true
        }
    }

    sourceSets["main"].java.srcDir("vendor/mcu")
}

kotlin {
    // Paparazzi 2.x requires a Java 21 toolchain.
    jvmToolchain(21)
}

dependencies {
    // The newest material3 alpha compatible with pre-AGP-9 builds: alpha19+
    // pull Compose 1.12 alphas that require AGP 9.1 and compileSdk 37, which
    // Paparazzi 2.x can't consume. 1.5.0-alpha18 carries all the Expressive
    // APIs (MaterialExpressiveTheme, MotionScheme.expressive()) on Compose
    // 1.11.0-beta02.
    // `implementation`, not `testImplementation`: Paparazzi's
    // `aarExplodedDirs` comes from the variant's runtime configuration, so
    // a testImplementation dep's resources (e.g. `default_error_message`,
    // resolved eagerly by every TextField) would be absent from the merged
    // resource table and crash composition with a NotFoundException.
    implementation("androidx.compose.material3:material3:1.5.0-alpha18")
    testImplementation("app.cash.paparazzi:paparazzi:2.0.0-alpha05")

    // Annotation-only deps of the vendored material-color-utilities
    // (vendor/mcu) — needed at compile time only.
    compileOnly("androidx.annotation:annotation:1.9.1")
    compileOnly("com.google.errorprone:error_prone_annotations:2.36.0")

    testImplementation("junit:junit:4.13.2")
    testImplementation(project(":harness"))
}

val renderCaseDirectory = layout.buildDirectory.dir("generated/parity-render-cases")
val generateParityRenderCases by tasks.registering {
    val sceneDirectory = layout.projectDirectory.dir("src/test/resources/scenes")
    val sceneFilter = providers.systemProperty("parity.scene").orElse("")
    val densityFilter = providers.systemProperty("parity.density").orElse("")
    inputs.dir(sceneDirectory)
    inputs.property("scene", sceneFilter)
    inputs.property("density", densityFilter)
    outputs.dir(renderCaseDirectory)
    doLast {
        val onlyScenes = sceneFilter.get().split(',').map { it.trim() }.filter { it.isNotEmpty() }.toSet()
        val onlyDensity = densityFilter.get().takeIf { it.isNotEmpty() }?.toInt()
        val directory = renderCaseDirectory.get().asFile
        directory.mkdirs()
        val sources = mutableMapOf<String, String>()
        val matchedScenes = mutableSetOf<String>()
        val index = sceneDirectory.file("index.txt").asFile.readLines().filter { it.isNotBlank() }
        for (entry in index) {
            val scene = JsonSlurper().parse(sceneDirectory.file("$entry.json").asFile) as Map<*, *>
            val name = scene["name"] as String
            require(name.matches(Regex("[a-z][a-z0-9_-]*"))) { "Invalid scene name: $name" }
            if (onlyScenes.isNotEmpty() && name !in onlyScenes) continue
            matchedScenes += name
            for (value in scene["densities"] as List<*>) {
                val density = (value as Number).toInt()
                if (onlyDensity != null && density != onlyDensity) continue
                val className = "RenderTest_${name.replace('-', '_')}_d$density"
                val fileName = "$className.java"
                require(fileName !in sources) { "Duplicate render case: $className" }
                sources[fileName] = """
                    package org.slint.material.parity;
                    public final class $className {
                        @org.junit.Test
                        public void render() {
                            new RenderTest("$name", $density).render();
                        }
                    }
                """.trimIndent() + "\n"
            }
        }
        val missingScenes = onlyScenes - matchedScenes
        require(missingScenes.isEmpty()) { "Unknown render scenes: ${missingScenes.joinToString()}" }
        require(sources.isNotEmpty()) { "No render scenes match the requested filters" }
        directory.listFiles()?.filter { it.extension == "java" && it.name !in sources }
            ?.forEach { check(it.delete()) { "Cannot remove stale render case: $it" } }
        sources.forEach { (name, source) -> directory.resolve(name).writeText(source) }
    }
}
android.sourceSets["test"].java.srcDir(renderCaseDirectory)
tasks.matching {
    it.name.startsWith("compile") &&
        (it.name.endsWith("UnitTestKotlin") || it.name.endsWith("UnitTestJavaWithJavac"))
}.configureEach { dependsOn(generateParityRenderCases) }

tasks.withType<Test>().configureEach {
    testLogging.showStandardStreams = true
    // Gradle forks per test class; generated classes keep each scene and density in a fresh JVM.
    forkEvery = 1
    providers.systemProperty("parity.scene").orNull?.let { systemProperty("parity.scene", it) }
    providers.systemProperty("parity.traceRender").orNull?.let { systemProperty("parity.traceRender", it) }
    // Paparazzi decompresses layoutlib natives at runtime.
    jvmArgs = (jvmArgs ?: emptyList()) + listOf(
        "--add-opens=java.base/java.lang=ALL-UNNAMED",
        "--add-opens=java.base/java.lang.reflect=ALL-UNNAMED",
        "--add-opens=java.base/java.util=ALL-UNNAMED",
        "--add-opens=java.base/java.io=ALL-UNNAMED",
        "--add-opens=java.desktop/java.awt=ALL-UNNAMED",
        "--add-opens=java.desktop/java.awt.font=ALL-UNNAMED",
    )
    // Where the harness writes renders: `references/` to regenerate the
    // committed references, `build/parity-out` otherwise.
    systemProperty(
        "parity.out.dir",
        findProperty("parity.record")?.let {
            File(rootDir, "references").absolutePath
        } ?: File(rootDir, "build/parity-out").absolutePath,
    )
}
