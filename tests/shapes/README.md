# Shape golden vectors

Golden test vectors for `internal/core/graphics/shapes/`, the Rust port of
[androidx.graphics.shapes](https://github.com/androidx/androidx) pinned at
commit `23327507f7fc7d5b19d65fec4b090f60c970079b`, with the morph engine
(`AngleMeasurer`, greedy `doMapping`, inclusive `validateProgress`, unwrapped
feature progress) taken from the packaged
`androidx.graphics:graphics-shapes:1.0.1` that Compose M3 Expressive depends
on.

- `src/androidx/` — Kotlin sources vendored from androidx
  (`graphics/shapes/` at the pin above, with the `PolygonMeasure.kt`,
  `FeatureMapping.kt`, `FloatMapping.kt` and `Morph.kt` morph-engine pieces
  aligned to graphics-shapes 1.0.1, plus `compose/ui/graphics/Matrix.kt`,
  `compose/material3/MaterialShapes.kt` and
  `compose/material3/internal/ShapeUtil.kt`, Apache-2.0 licensed, upstream
  license headers retained).
- `src/stubs/` — minimal semantic stubs for the bits of Compose and the Kotlin
  stdlib the vendored sources touch, so they compile standalone.
- `src/GoldenGenerator.kt` — entry point: builds every `MaterialShapes` member
  and a set of primitive constructors, plus sampled morphs, and writes them as
  JSON with every f32 encoded as its raw IEEE-754 bit pattern.
- `golden/` — the generated vectors, consumed by
  `internal/core/tests/shapes_parity.rs`.

## Regenerating

Needs `kotlinc` (tested with 2.1.21) and a JRE:

```sh
./generate.sh
```

The output is deterministic; any diff means either the vendored sources, the
generator, or the Kotlin toolchain changed — never edit the JSON by hand.
