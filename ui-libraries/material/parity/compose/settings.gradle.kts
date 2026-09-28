// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

pluginManagement {
    repositories {
        // Google Maven must come first: plugins.gradle.org redirects Android
        // plugin markers to repo.maven.apache.org, which rate-limits some
        // CI hosts. Kotlin plugin markers resolve from the Central mirrors.
        maven { url = uri("https://dl.google.com/dl/android/maven2/") }
        maven { url = uri("https://maven.aliyun.com/repository/central") }
        maven { url = uri("https://cache-redirector.jetbrains.com/maven-central") }
        maven { url = uri("https://maven.aliyun.com/repository/gradle-plugin") }
        maven { url = uri("https://plugins.gradle.org/m2/") }
    }
}

dependencyResolutionManagement {
    repositories {
        maven { url = uri("https://dl.google.com/dl/android/maven2/") }
        maven { url = uri("https://maven.aliyun.com/repository/central") }
        maven { url = uri("https://cache-redirector.jetbrains.com/maven-central") }
        google()
        mavenCentral()
    }
}

rootProject.name = "material-parity-compose"
