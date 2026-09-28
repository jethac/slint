// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

plugins {
    // AGP 9.1 embeds Kotlin support; no org.jetbrains.kotlin.android plugin.
    id("com.android.library") version "9.2.1"
    kotlin("plugin.compose") version "2.4.20"
}

// Rendering choice, recorded for #4: Robolectric with
// `graphicsMode = NATIVE` (real rasterization through the Android native
// graphics stack) instead of Paparazzi/layoutlib, because the harness needs
// the test-runtime clock (`mainClock.autoAdvance = false` plus
// `advanceTimeBy`) and pointer input injection, which the compose-ui-test
// rule provides under Robolectric. Paparazzi renders one composed frame per
// invocation and has no manual-clock animation stepping or
// `performTouchInput`, so it cannot produce the motion frames and traces this
// harness needs.

android {
    namespace = "org.slint.material.parity"
    compileSdk = 37
    defaultConfig {
        minSdk = 26
    }
    buildFeatures {
        compose = true
    }
    testOptions {
        unitTests {
            // Scenes, fonts and robolectric.properties live under
            // src/test/resources and must reach the unit test classpath.
            isIncludeAndroidResources = true
            isReturnDefaultValues = true
        }
    }
}

kotlin {
    jvmToolchain(17)
}

dependencies {
    testImplementation("androidx.compose.material3:material3:1.5.0-alpha29")
    // Compose 1.13.0-alpha01 is what material3:1.5.0-alpha29 resolves to;
    // keep the test rule on the same line.
    testImplementation("androidx.compose.ui:ui-test-junit4:1.13.0-alpha01")
    testImplementation("androidx.activity:activity-compose:1.11.0")
    testImplementation("org.robolectric:robolectric:4.17")
    testImplementation("junit:junit:4.13.2")
}

tasks.withType<Test>().configureEach {
    // Robolectric on JDK 17 needs the JPMS opens it documents; and its runtime
    // dependency resolver (the android-all jars) talks to Maven Central
    // directly, so point it at the mirror the Gradle repositories use.
    jvmArgs = (jvmArgs ?: emptyList()) + listOf(
        "--add-opens=java.base/java.lang=ALL-UNNAMED",
        "--add-opens=java.base/java.lang.reflect=ALL-UNNAMED",
        "--add-opens=java.base/java.util=ALL-UNNAMED",
        "--add-opens=java.base/java.util.concurrent=ALL-UNNAMED",
        "--add-opens=java.base/java.io=ALL-UNNAMED",
        "--add-opens=java.base/java.nio=ALL-UNNAMED",
        "--add-opens=java.base/sun.nio.ch=ALL-UNNAMED",
        "--add-opens=java.base/java.net=ALL-UNNAMED",
        "--add-opens=java.desktop/java.awt=ALL-UNNAMED",
        "--add-opens=java.desktop/java.awt.font=ALL-UNNAMED",
    )
    systemProperty("robolectric.dependency.repo.url", "https://maven.aliyun.com/repository/central")
    systemProperty("robolectric.dependency.repo.id", "aliyun-central")
    // Where the harness writes renders: `references/` to regenerate the
    // committed references, `build/parity-out` otherwise.
    systemProperty(
        "parity.out.dir",
        findProperty("parity.record")?.let { "references" } ?: "build/parity-out",
    )
}
