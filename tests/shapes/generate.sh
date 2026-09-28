#!/bin/sh
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Regenerates golden/material_shapes.json and golden/morphs.json by running the
# vendored Kotlin sources (see README.md). Requires kotlinc and java on PATH
# (tested with kotlinc 2.1.21).

set -e
cd "$(dirname "$0")"

OUT_JAR="$(mktemp -d)/golden-gen.jar"

kotlinc \
    src/androidx/graphics/shapes/*.kt \
    src/androidx/compose/ui/graphics/Matrix.kt \
    src/androidx/compose/material3/MaterialShapes.kt \
    src/androidx/compose/material3/internal/ShapeUtil.kt \
    src/stubs/*.kt \
    src/GoldenGenerator.kt \
    -include-runtime -d "$OUT_JAR" -nowarn

java -jar "$OUT_JAR" golden
