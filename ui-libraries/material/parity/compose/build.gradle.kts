// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

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
    testImplementation("androidx.compose.material3:material3:1.5.0-alpha18")
    testImplementation("app.cash.paparazzi:paparazzi:2.0.0-alpha05")

    // Annotation-only deps of the vendored material-color-utilities
    // (vendor/mcu) — needed at compile time only.
    compileOnly("androidx.annotation:annotation:1.9.1")
    compileOnly("com.google.errorprone:error_prone_annotations:2.36.0")

    testImplementation("junit:junit:4.13.2")
    testImplementation(project(":harness"))
}

tasks.withType<Test>().configureEach {
    testLogging.showStandardStreams = true
    // One JVM per render: composition state that survives teardown (the
    // frame clock keeps its epoch across render sessions) can freeze a
    // later render's coroutine-driven animations in a shared JVM.
    forkEvery = 1
    providers.systemProperty("parity.scene").orNull?.let { systemProperty("parity.scene", it) }
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
