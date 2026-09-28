// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// A plain java-library module for the PNG/frame-sink helpers: they name
// java.awt types, but AGP compiles Android unit-test sources against its
// whittled JDK image (androidJdkImage), which drops java.desktop. A normal
// `java` module compiles against the real JDK.
plugins {
    `java-library`
}

java {
    toolchain {
        languageVersion.set(JavaLanguageVersion.of(21))
    }
}

dependencies {
    api("app.cash.paparazzi:paparazzi:2.0.0-alpha05")
}
