// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

pluginManagement {
    repositories {
        google()
        // Google's read-only mirror of Maven Central: repo.maven.apache.org
        // rate-limits build agents, the mirror does not.
        maven("https://maven-central.storage-download.googleapis.com/maven2/")
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositories {
        google()
        maven("https://maven-central.storage-download.googleapis.com/maven2/")
        mavenCentral()
    }
}

rootProject.name = "material-parity-compose"

include(":harness")
