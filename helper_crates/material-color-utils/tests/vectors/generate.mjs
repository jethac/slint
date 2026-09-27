// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Generates bit-exact test vectors for material-color-utils by running the
// TypeScript implementation of material-color-utilities at the pinned commit
// (material-foundation/material-color-utilities @
// 5b3618b16fdc3825e21d5679bafd144662088ea1).
//
// Usage:
//   # in a checkout of the pinned upstream repo:
//   tsc --ignoreConfig --outDir /tmp/mcu-js --rootDir typescript \
//       --module nodenext --target es2022 --moduleResolution nodenext \
//       --skipLibCheck --noEmitOnError false typescript/index.ts
//   # fix the few relative imports tsc emitted without a .js extension:
//   find /tmp/mcu-js -name '*.js' -exec sed -i -E \
//       "s|from '(\.{1,2}/[^']*[^/a-zA-Z0-9_.]?)'|from '\1.js'|g" {} +
//   MCU_JS_DIR=/tmp/mcu-js node generate.mjs | gzip -9 > vectors.txt.gz
//
// Format: one vector per line, "<kind> <key-fields> = <outputs>".
// Floating-point outputs are encoded as their IEEE-754 bit pattern in hex
// (prefix "f:") so that comparison is bit-exact.

const dir = process.env.MCU_JS_DIR;
if (!dir) {
  console.error('set MCU_JS_DIR to the compiled TypeScript output dir');
  process.exit(1);
}
const m = await import(`${dir}/index.js`);

// The TypeScript QuantizerWsmeans draws random starting state with
// Math.random(), which makes its output non-deterministic. The Kotlin
// implementation instead uses a seeded java.util.Random(0x42688) "to ensure
// consistent results". So that the generated vectors are reproducible and
// checkable by the Rust port, Math.random is replaced by a port of
// java.util.Random with the same seed. The Rust crate implements the same
// generator in quantize/java_random.rs and consumes values in the same order.
class JavaRandom {
  constructor(seed) {
    this.seed = (BigInt(seed) ^ 0x5DEECE66Dn) & ((1n << 48n) - 1n);
  }
  next(bits) {
    this.seed = (this.seed * 0x5DEECE66Dn + 0xBn) & ((1n << 48n) - 1n);
    return Number(this.seed >> (48n - BigInt(bits)));
  }
  nextDouble() {
    return ((this.next(26) * 2 ** 27) + this.next(27)) / 2 ** 53;
  }
}
let rng = new JavaRandom(0x42688);
Math.random = () => rng.nextDouble();
const resetRng = () => {
  rng = new JavaRandom(0x42688);
};

const TOKENS = [
  'primaryPaletteKeyColor', 'secondaryPaletteKeyColor', 'tertiaryPaletteKeyColor',
  'neutralPaletteKeyColor', 'neutralVariantPaletteKeyColor', 'errorPaletteKeyColor',
  'background', 'onBackground', 'surface', 'surfaceDim', 'surfaceBright',
  'surfaceContainerLowest', 'surfaceContainerLow', 'surfaceContainer',
  'surfaceContainerHigh', 'surfaceContainerHighest', 'onSurface', 'surfaceVariant',
  'onSurfaceVariant', 'inverseSurface', 'inverseOnSurface', 'outline',
  'outlineVariant', 'shadow', 'scrim', 'surfaceTint', 'primary', 'primaryDim',
  'onPrimary', 'primaryContainer', 'onPrimaryContainer', 'primaryFixed',
  'primaryFixedDim', 'onPrimaryFixed', 'onPrimaryFixedVariant', 'inversePrimary',
  'secondary', 'secondaryDim', 'onSecondary', 'secondaryContainer',
  'onSecondaryContainer', 'secondaryFixed', 'secondaryFixedDim', 'onSecondaryFixed',
  'onSecondaryFixedVariant', 'tertiary', 'tertiaryDim', 'onTertiary',
  'tertiaryContainer', 'onTertiaryContainer', 'tertiaryFixed', 'tertiaryFixedDim',
  'onTertiaryFixed', 'onTertiaryFixedVariant', 'error', 'errorDim', 'onError',
  'errorContainer', 'onErrorContainer',
];

const hex = (n) => '0x' + (n >>> 0).toString(16).padStart(8, '0');
const fbits = (x) => {
  const b = new Float64Array([x]);
  const u = new BigUint64Array(b.buffer);
  return 'f:' + u[0].toString(16).padStart(16, '0');
};

const SEEDS = [0xff4285f4, 0xff6750a4, 0xff0000ff, 0xffff0000, 0xff00ff00, 0xffd0bcff];
const SECOND_SEEDS = [0xff006877, 0xff9a25ae, 0xff00ff00, 0xff4285f4, 0xff123456, 0xffff8800];
const VARIANTS = ['monochrome', 'neutral', 'tonal_spot', 'vibrant', 'expressive',
  'fidelity', 'content', 'rainbow', 'fruit_salad', 'cmf'];
const VARIANT_CTOR = {
  monochrome: m.SchemeMonochrome, neutral: m.SchemeNeutral, tonal_spot: m.SchemeTonalSpot,
  vibrant: m.SchemeVibrant, expressive: m.SchemeExpressive, fidelity: m.SchemeFidelity,
  content: m.SchemeContent, rainbow: m.SchemeRainbow, fruit_salad: m.SchemeFruitSalad,
  cmf: m.SchemeCmf,
};
const VARIANT_ENUM = {
  monochrome: m.Variant.MONOCHROME, neutral: m.Variant.NEUTRAL,
  tonal_spot: m.Variant.TONAL_SPOT, vibrant: m.Variant.VIBRANT,
  expressive: m.Variant.EXPRESSIVE, fidelity: m.Variant.FIDELITY,
  content: m.Variant.CONTENT, rainbow: m.Variant.RAINBOW,
  fruit_salad: m.Variant.FRUIT_SALAD, cmf: m.Variant.CMF,
};
const SPECS = ['2021', '2025', '2026'];
const CONTRASTS = [-1.0, -0.5, 0.0, 0.5, 1.0];
const PLATFORMS = ['phone', 'watch'];
const PENUM = { phone: 'phone', watch: 'watch' };

const out = [];
for (let si = 0; si < SEEDS.length; si++) {
  const seed = SEEDS[si];
  for (const variant of VARIANTS) {
    for (const spec of SPECS) {
      if (variant === 'cmf' && spec !== '2026') continue;
      for (const cl of CONTRASTS) {
        for (const dark of [false, true]) {
          for (const platform of PLATFORMS) {
            const hcts = variant === 'cmf'
              ? [m.Hct.fromInt(seed), m.Hct.fromInt(SECOND_SEEDS[si])]
              : m.Hct.fromInt(seed);
            let scheme;
            try {
              scheme = new VARIANT_CTOR[variant](hcts, dark, cl, spec, PENUM[platform]);
            } catch (e) {
              continue;
            }
            const key = [variant, spec, cl, dark ? 1 : 0, platform,
              hex(seed), variant === 'cmf' ? hex(SECOND_SEEDS[si]) : '-'].join(' ');
            for (const t of TOKENS) {
              out.push(`scheme ${key} ${t} = ${hex(scheme[t])}`);
            }
          }
        }
      }
    }
  }
}

// HCT primitives: argb -> hue/chroma/tone (bit-exact doubles) and round-trip.
const PRIM_SEEDS = [...SEEDS, ...SECOND_SEEDS, 0xff000000, 0xffffffff, 0xff7f7f7f,
  0xff123456, 0xffdeadbe, 0xff00697c];
for (const argb of PRIM_SEEDS) {
  const h = m.Hct.fromInt(argb);
  out.push(`hct ${hex(argb)} = ${fbits(h.hue)} ${fbits(h.chroma)} ${fbits(h.tone)} ${hex(h.toInt())}`);
}
for (const [hue, chroma, tone] of [[25, 84, 50], [270, 40, 60], [150, 100, 30],
    [0, 0, 50], [359, 120, 90], [210, 5, 10], [48, 200, 99], [200, 30, 87.5]]) {
  const h = m.Hct.from(hue, chroma, tone);
  out.push(`hct_from ${hue} ${chroma} ${tone} = ${hex(h.toInt())}`);
}

// Tonal palettes: from argb, tones 0..100.
for (const argb of PRIM_SEEDS.slice(0, 8)) {
  const p = m.TonalPalette.fromInt(argb);
  const tones = [];
  for (let t = 0; t <= 100; t++) tones.push(hex(p.tone(t)));
  out.push(`palette ${hex(argb)} = ${tones.join(' ')}`);
}
for (const [hue, chroma] of [[25, 84], [270, 40], [150, 100], [200, 3], [0, 0], [90, 60]]) {
  const p = m.TonalPalette.fromHueAndChroma(hue, chroma);
  const tones = [];
  for (let t = 0; t <= 100; t++) tones.push(hex(p.tone(t)));
  out.push(`palette_hc ${hue} ${chroma} = ${tones.join(' ')}`);
}

// Blend, contrast, dislike, temperature.
for (const a of PRIM_SEEDS) {
  for (const b of PRIM_SEEDS.slice(0, 4)) {
    out.push(`harmonize ${hex(a)} ${hex(b)} = ${hex(m.Blend.harmonize(a, b))}`);
    out.push(`cam16_ucs ${hex(a)} ${hex(b)} = ${hex(m.Blend.cam16Ucs(a, b, 0.5))}`);
    out.push(`hct_hue ${hex(a)} ${hex(b)} = ${hex(m.Blend.hctHue(a, b, 90))}`);
    out.push(`dislike ${hex(a)} ${hex(b)} = ${hex(m.DislikeAnalyzer.fixIfDisliked(m.Hct.fromInt(a)).toInt())}`);
  }
  const cache = new m.TemperatureCache(m.Hct.fromInt(a));
  const ana = cache.analogous(5).map((h) => hex(h.toInt())).join(' ');
  out.push(`temperature ${hex(a)} = ${hex(cache.complement.toInt())} ${ana}`);
}
for (const [a, b] of [[0, 100], [50, 50], [10, 99], [0, 0], [100, 100], [30, 70], [5, 95]]) {
  out.push(`contrast_tone ${a} ${b} = ${fbits(m.Contrast.ratioOfTones(a, b))}`);
}
for (const tone of [0, 5, 10, 25, 40, 50, 60, 75, 90, 95, 99, 100]) {
  for (const ratio of [1.0, 1.5, 2.0, 3.0, 4.5, 7.0]) {
    const l = m.Contrast.lighterUnsafe(tone, ratio);
    const d = m.Contrast.darkerUnsafe(tone, ratio);
    out.push(`contrast_ld ${tone} ${ratio} = ${fbits(l)} ${fbits(d)}`);
  }
}

// Quantizer + scorer on fixed synthetic pixel arrays.
const IMAGES = [
  [0xff4285f4, 0xff4285f4, 0xff006877, 0xff9a25ae, 0xff00ff00, 0xff123456,
    0xffff0000, 0xff0000ff, 0xffd0bcff, 0xff7f7f7f, 0xff4285f4, 0xff006877],
  [0xff000000, 0xffffffff, 0xff000000, 0xffffffff, 0xffff0000, 0xff00ff00,
    0xff0000ff, 0xffffff00, 0xff00ffff, 0xffff00ff],
  Array.from({length: 200}, (_, i) => 0xff000000 | ((i * 37) << 16 & 0xff0000) | ((i * 17) << 8 & 0xff00) | (i * 7 & 0xff)),
];
for (const img of IMAGES) {
  for (const maxColors of [8, 32, 128]) {
    resetRng(); // JavaRandom(0x42688) is created inside each quantize() call.
    const res = m.QuantizerCelebi.quantize(img, maxColors);
    const entries = [...res.entries()].sort((a, b) => a[0] - b[0])
        .map(([c, n]) => `${hex(c)}:${n}`).join(' ');
    out.push(`quantize ${maxColors} ${img.map(hex).join(',')} = ${entries}`);
    const scored = m.Score.score(res);
    out.push(`score ${maxColors} ${img.map(hex).join(',')} = ${scored.map(hex).join(' ')}`);
  }
}

process.stdout.write(out.join('\n') + '\n');
