#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Regenerates vectors.txt.gz from the Java implementation of
# material-color-utilities at the pinned commit
# (material-foundation/material-color-utilities @
# 5b3618b16fdc3825e21d5679bafd144662088ea1). Java is the authoritative
# reference for this crate (it backs Android's platform dynamic color).
#
# Usage: ./generate.sh /path/to/material-color-utilities
#
# The Java sources are copied and `Math.` calls are rewritten to
# `StrictMath.`: on HotSpot, `Math.pow`/`Math.exp`/`Math.log`/... are
# JIT intrinsics whose results differ from StrictMath by up to 1 ulp and
# are platform/JVM dependent. StrictMath is the canonical fdlibm profile —
# exactly what the `libm` crate implements — so the generated vectors are
# stable regardless of the JDK or CPU used to produce them.

set -euo pipefail

SRC="${1:?usage: $0 /path/to/material-color-utilities}"
HERE="$(cd "$(dirname "$0")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

cp -r "$SRC/java" "$WORK/java"
# Rewrite Math.* -> StrictMath.* (MathUtils. is not matched).
find "$WORK/java" -name '*.java' -exec sed -i 's/\bMath\./StrictMath./g' {} +

# Stubs for the AndroidX/ErrorProne annotations the Java sources use;
# they carry no semantics.
mkdir -p "$WORK/stubs/androidx/annotation" "$WORK/stubs/com/google/errorprone/annotations"
for ann in NonNull Nullable; do
    printf 'package androidx.annotation;\npublic @interface %s {}\n' "$ann" \
        > "$WORK/stubs/androidx/annotation/$ann.java"
done
for ann in CanIgnoreReturnValue CheckReturnValue Var; do
    printf 'package com.google.errorprone.annotations;\npublic @interface %s {}\n' "$ann" \
        > "$WORK/stubs/com/google/errorprone/annotations/$ann.java"
done

javac -d "$WORK/classes" $(find "$WORK/stubs" "$WORK/java" -name '*.java')
javac -cp "$WORK/classes" -d "$WORK/classes" "$HERE/Generate.java"
java -cp "$WORK/classes" Generate | gzip -9 > "$HERE/vectors.txt.gz"

echo "Wrote $HERE/vectors.txt.gz"
