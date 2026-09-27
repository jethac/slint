// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Generates bit-exact test vectors for material-color-utils by running the
// Java implementation of material-color-utilities at the pinned commit
// (material-foundation/material-color-utilities @
// 5b3618b16fdc3825e21d5679bafd144662088ea1). Java is the authoritative
// reference for this crate (it backs Android's platform dynamic color).
//
// Usage: ./generate.sh /path/to/material-color-utilities   (pinned commit)
// The script compiles the Java sources with Math.* rewritten to
// StrictMath.* — on HotSpot the Math.* forms are JIT intrinsics whose
// results differ from StrictMath by up to 1 ulp and are JVM/platform
// dependent, while StrictMath is the canonical fdlibm profile that the
// `libm` crate reproduces exactly.
//
// Format: one vector per line, "<kind> <key-fields> = <outputs>".
// Floating-point outputs are encoded as their IEEE-754 bit pattern in hex
// (prefix "f:") so that comparison is bit-exact.
//
// The TypeScript port's QuantizerWsmeans draws random starting state with
// unseeded Math.random(); the Java implementation uses a seeded
// java.util.Random(0x42688) "to ensure consistent results", so no seeding
// is needed here. The Rust crate implements the same generator in
// quantize/java_random.rs.

import blend.Blend;
import contrast.Contrast;
import dislike.DislikeAnalyzer;
import dynamiccolor.ColorSpec.SpecVersion;
import dynamiccolor.DynamicColor;
import dynamiccolor.DynamicScheme;
import dynamiccolor.DynamicScheme.Platform;
import dynamiccolor.MaterialDynamicColors;
import hct.Hct;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import palettes.TonalPalette;
import quantize.QuantizerCelebi;
import scheme.SchemeCmf;
import scheme.SchemeContent;
import scheme.SchemeExpressive;
import scheme.SchemeFidelity;
import scheme.SchemeFruitSalad;
import scheme.SchemeMonochrome;
import scheme.SchemeNeutral;
import scheme.SchemeRainbow;
import scheme.SchemeTonalSpot;
import scheme.SchemeVibrant;
import score.Score;
import temperature.TemperatureCache;

public class Generate {

    static final String[] TOKENS = {
        "primaryPaletteKeyColor", "secondaryPaletteKeyColor", "tertiaryPaletteKeyColor",
        "neutralPaletteKeyColor", "neutralVariantPaletteKeyColor", "errorPaletteKeyColor",
        "background", "onBackground", "surface", "surfaceDim", "surfaceBright",
        "surfaceContainerLowest", "surfaceContainerLow", "surfaceContainer",
        "surfaceContainerHigh", "surfaceContainerHighest", "onSurface", "surfaceVariant",
        "onSurfaceVariant", "inverseSurface", "inverseOnSurface", "outline",
        "outlineVariant", "shadow", "scrim", "surfaceTint", "primary", "primaryDim",
        "onPrimary", "primaryContainer", "onPrimaryContainer", "primaryFixed",
        "primaryFixedDim", "onPrimaryFixed", "onPrimaryFixedVariant", "inversePrimary",
        "secondary", "secondaryDim", "onSecondary", "secondaryContainer",
        "onSecondaryContainer", "secondaryFixed", "secondaryFixedDim", "onSecondaryFixed",
        "onSecondaryFixedVariant", "tertiary", "tertiaryDim", "onTertiary",
        "tertiaryContainer", "onTertiaryContainer", "tertiaryFixed", "tertiaryFixedDim",
        "onTertiaryFixed", "onTertiaryFixedVariant", "error", "errorDim", "onError",
        "errorContainer", "onErrorContainer",
    };

    static DynamicColor tokenColor(MaterialDynamicColors c, String token) {
        switch (token) {
            case "primaryPaletteKeyColor": return c.primaryPaletteKeyColor();
            case "secondaryPaletteKeyColor": return c.secondaryPaletteKeyColor();
            case "tertiaryPaletteKeyColor": return c.tertiaryPaletteKeyColor();
            case "neutralPaletteKeyColor": return c.neutralPaletteKeyColor();
            case "neutralVariantPaletteKeyColor": return c.neutralVariantPaletteKeyColor();
            case "errorPaletteKeyColor": return c.errorPaletteKeyColor();
            case "background": return c.background();
            case "onBackground": return c.onBackground();
            case "surface": return c.surface();
            case "surfaceDim": return c.surfaceDim();
            case "surfaceBright": return c.surfaceBright();
            case "surfaceContainerLowest": return c.surfaceContainerLowest();
            case "surfaceContainerLow": return c.surfaceContainerLow();
            case "surfaceContainer": return c.surfaceContainer();
            case "surfaceContainerHigh": return c.surfaceContainerHigh();
            case "surfaceContainerHighest": return c.surfaceContainerHighest();
            case "onSurface": return c.onSurface();
            case "surfaceVariant": return c.surfaceVariant();
            case "onSurfaceVariant": return c.onSurfaceVariant();
            case "inverseSurface": return c.inverseSurface();
            case "inverseOnSurface": return c.inverseOnSurface();
            case "outline": return c.outline();
            case "outlineVariant": return c.outlineVariant();
            case "shadow": return c.shadow();
            case "scrim": return c.scrim();
            case "surfaceTint": return c.surfaceTint();
            case "primary": return c.primary();
            case "primaryDim": return c.primaryDim();
            case "onPrimary": return c.onPrimary();
            case "primaryContainer": return c.primaryContainer();
            case "onPrimaryContainer": return c.onPrimaryContainer();
            case "primaryFixed": return c.primaryFixed();
            case "primaryFixedDim": return c.primaryFixedDim();
            case "onPrimaryFixed": return c.onPrimaryFixed();
            case "onPrimaryFixedVariant": return c.onPrimaryFixedVariant();
            case "inversePrimary": return c.inversePrimary();
            case "secondary": return c.secondary();
            case "secondaryDim": return c.secondaryDim();
            case "onSecondary": return c.onSecondary();
            case "secondaryContainer": return c.secondaryContainer();
            case "onSecondaryContainer": return c.onSecondaryContainer();
            case "secondaryFixed": return c.secondaryFixed();
            case "secondaryFixedDim": return c.secondaryFixedDim();
            case "onSecondaryFixed": return c.onSecondaryFixed();
            case "onSecondaryFixedVariant": return c.onSecondaryFixedVariant();
            case "tertiary": return c.tertiary();
            case "tertiaryDim": return c.tertiaryDim();
            case "onTertiary": return c.onTertiary();
            case "tertiaryContainer": return c.tertiaryContainer();
            case "onTertiaryContainer": return c.onTertiaryContainer();
            case "tertiaryFixed": return c.tertiaryFixed();
            case "tertiaryFixedDim": return c.tertiaryFixedDim();
            case "onTertiaryFixed": return c.onTertiaryFixed();
            case "onTertiaryFixedVariant": return c.onTertiaryFixedVariant();
            case "error": return c.error();
            case "errorDim": return c.errorDim();
            case "onError": return c.onError();
            case "errorContainer": return c.errorContainer();
            case "onErrorContainer": return c.onErrorContainer();
            default: throw new IllegalArgumentException("unknown token " + token);
        }
    }

    static final int[] SEEDS = {0xff4285f4, 0xff6750a4, 0xff0000ff, 0xffff0000, 0xff00ff00,
        0xffd0bcff};
    static final int[] SECOND_SEEDS = {0xff006877, 0xff9a25ae, 0xff00ff00, 0xff4285f4, 0xff123456,
        0xffff8800};
    static final String[] VARIANTS = {"monochrome", "neutral", "tonal_spot", "vibrant",
        "expressive", "fidelity", "content", "rainbow", "fruit_salad", "cmf"};
    static final String[] SPECS = {"2021", "2025", "2026"};
    static final double[] CONTRASTS = {-1.0, -0.5, 0.0, 0.5, 1.0};
    static final String[] PLATFORMS = {"phone", "watch"};

    static String hex(int n) {
        return String.format("0x%08x", n);
    }

    static String fbits(double x) {
        return String.format("f:%016x", Double.doubleToRawLongBits(x));
    }

    // Render a double like JavaScript does in these vectors: integral values
    // without a decimal point ("-1"), the rest with the shortest round-trip
    // representation ("-0.5").
    static String num(double x) {
        if (x == Math.floor(x) && !Double.isInfinite(x)) {
            return String.valueOf((long) x);
        }
        return String.valueOf(x);
    }

    static SpecVersion spec(String name) {
        switch (name) {
            case "2021": return SpecVersion.SPEC_2021;
            case "2025": return SpecVersion.SPEC_2025;
            case "2026": return SpecVersion.SPEC_2026;
            default: throw new IllegalArgumentException("spec " + name);
        }
    }

    static Platform platform(String name) {
        switch (name) {
            case "phone": return Platform.PHONE;
            case "watch": return Platform.WATCH;
            default: throw new IllegalArgumentException("platform " + name);
        }
    }

    static DynamicScheme makeScheme(String variant, List<Hct> hcts, boolean dark, double cl,
            SpecVersion spec, Platform platform) {
        switch (variant) {
            case "monochrome": return new SchemeMonochrome(hcts, dark, cl, spec, platform);
            case "neutral": return new SchemeNeutral(hcts, dark, cl, spec, platform);
            case "tonal_spot": return new SchemeTonalSpot(hcts, dark, cl, spec, platform);
            case "vibrant": return new SchemeVibrant(hcts, dark, cl, spec, platform);
            case "expressive": return new SchemeExpressive(hcts, dark, cl, spec, platform);
            case "fidelity": return new SchemeFidelity(hcts, dark, cl, spec, platform);
            case "content": return new SchemeContent(hcts, dark, cl, spec, platform);
            case "rainbow": return new SchemeRainbow(hcts, dark, cl, spec, platform);
            case "fruit_salad": return new SchemeFruitSalad(hcts, dark, cl, spec, platform);
            case "cmf": return new SchemeCmf(hcts, dark, cl, spec, platform);
            default: throw new IllegalArgumentException("variant " + variant);
        }
    }

    public static void main(String[] args) {
        List<String> out = new ArrayList<>();

        for (int si = 0; si < SEEDS.length; si++) {
            int seed = SEEDS[si];
            for (String variant : VARIANTS) {
                for (String specName : SPECS) {
                    if (variant.equals("cmf") && !specName.equals("2026")) {
                        continue;
                    }
                    for (double cl : CONTRASTS) {
                        for (boolean dark : new boolean[] {false, true}) {
                            for (String platformName : PLATFORMS) {
                                List<Hct> hcts = new ArrayList<>();
                                hcts.add(Hct.fromInt(seed));
                                if (variant.equals("cmf")) {
                                    hcts.add(Hct.fromInt(SECOND_SEEDS[si]));
                                }
                                DynamicScheme scheme;
                                try {
                                    scheme = makeScheme(variant, hcts, dark, cl,
                                            spec(specName), platform(platformName));
                                } catch (RuntimeException e) {
                                    continue;
                                }
                                MaterialDynamicColors colors = new MaterialDynamicColors();
                                String key = String.join(" ", variant, specName, num(cl),
                                        dark ? "1" : "0", platformName, hex(seed),
                                        variant.equals("cmf") ? hex(SECOND_SEEDS[si]) : "-");
                                for (String t : TOKENS) {
                                    out.add("scheme " + key + " " + t + " = "
                                            + hex(scheme.getArgb(tokenColor(colors, t))));
                                }
                            }
                        }
                    }
                }
            }
        }

        // HCT primitives: argb -> hue/chroma/tone (bit-exact doubles) and round-trip.
        int[] primSeeds = {SEEDS[0], SEEDS[1], SEEDS[2], SEEDS[3], SEEDS[4], SEEDS[5],
            SECOND_SEEDS[0], SECOND_SEEDS[1], SECOND_SEEDS[2], SECOND_SEEDS[3], SECOND_SEEDS[4],
            SECOND_SEEDS[5], 0xff000000, 0xffffffff, 0xff7f7f7f, 0xff123456, 0xffdeadbe,
            0xff00697c};
        for (int argb : primSeeds) {
            Hct h = Hct.fromInt(argb);
            out.add("hct " + hex(argb) + " = " + fbits(h.getHue()) + " " + fbits(h.getChroma())
                    + " " + fbits(h.getTone()) + " " + hex(h.toInt()));
        }
        double[][] hctTriples = {
            {25, 84, 50}, {270, 40, 60}, {150, 100, 30}, {0, 0, 50},
            {359, 120, 90}, {210, 5, 10}, {48, 200, 99}, {200, 30, 87.5}};
        for (double[] t : hctTriples) {
            Hct h = Hct.from(t[0], t[1], t[2]);
            out.add("hct_from " + num(t[0]) + " " + num(t[1]) + " " + num(t[2]) + " = "
                    + hex(h.toInt()));
        }

        // Tonal palettes: from argb, tones 0..100.
        for (int i = 0; i < 8; i++) {
            TonalPalette p = TonalPalette.fromInt(primSeeds[i]);
            List<String> tones = new ArrayList<>();
            for (int t = 0; t <= 100; t++) {
                tones.add(hex(p.tone(t)));
            }
            out.add("palette " + hex(primSeeds[i]) + " = " + String.join(" ", tones));
        }
        double[][] hcPairs = {{25, 84}, {270, 40}, {150, 100}, {200, 3}, {0, 0}, {90, 60}};
        for (double[] hc : hcPairs) {
            TonalPalette p = TonalPalette.fromHueAndChroma(hc[0], hc[1]);
            List<String> tones = new ArrayList<>();
            for (int t = 0; t <= 100; t++) {
                tones.add(hex(p.tone(t)));
            }
            out.add("palette_hc " + num(hc[0]) + " " + num(hc[1]) + " = "
                    + String.join(" ", tones));
        }

        // Blend, contrast, dislike, temperature.
        for (int a : primSeeds) {
            for (int i = 0; i < 4; i++) {
                int b = primSeeds[i];
                out.add("harmonize " + hex(a) + " " + hex(b) + " = "
                        + hex(Blend.harmonize(a, b)));
                out.add("cam16_ucs " + hex(a) + " " + hex(b) + " = "
                        + hex(Blend.cam16Ucs(a, b, 0.5)));
                out.add("hct_hue " + hex(a) + " " + hex(b) + " = "
                        + hex(Blend.hctHue(a, b, 90)));
                out.add("dislike " + hex(a) + " " + hex(b) + " = "
                        + hex(DislikeAnalyzer.fixIfDisliked(Hct.fromInt(a)).toInt()));
            }
            TemperatureCache cache = new TemperatureCache(Hct.fromInt(a));
            List<String> ana = new ArrayList<>();
            for (Hct h : cache.getAnalogousColors(5, 12)) {
                ana.add(hex(h.toInt()));
            }
            out.add("temperature " + hex(a) + " = " + hex(cache.getComplement().toInt()) + " "
                    + String.join(" ", ana));
        }
        int[][] tonePairs = {{0, 100}, {50, 50}, {10, 99}, {0, 0}, {100, 100}, {30, 70},
            {5, 95}};
        for (int[] ab : tonePairs) {
            out.add("contrast_tone " + ab[0] + " " + ab[1] + " = "
                    + fbits(Contrast.ratioOfTones(ab[0], ab[1])));
        }
        int[] tones = {0, 5, 10, 25, 40, 50, 60, 75, 90, 95, 99, 100};
        double[] ratios = {1.0, 1.5, 2.0, 3.0, 4.5, 7.0};
        for (int tone : tones) {
            for (double ratio : ratios) {
                double l = Contrast.lighterUnsafe(tone, ratio);
                double d = Contrast.darkerUnsafe(tone, ratio);
                out.add("contrast_ld " + tone + " " + num(ratio) + " = " + fbits(l) + " "
                        + fbits(d));
            }
        }

        // Quantizer + scorer on fixed synthetic pixel arrays.
        int[][] images = {{0xff4285f4, 0xff4285f4, 0xff006877, 0xff9a25ae, 0xff00ff00,
                0xff123456, 0xffff0000, 0xff0000ff, 0xffd0bcff, 0xff7f7f7f, 0xff4285f4,
                0xff006877},
            {0xff000000, 0xffffffff, 0xff000000, 0xffffffff, 0xffff0000, 0xff00ff00,
                0xff0000ff, 0xffffff00, 0xff00ffff, 0xffff00ff},
            new int[200]};
        for (int i = 0; i < 200; i++) {
            images[2][i] = 0xff000000 | ((i * 37) << 16 & 0xff0000) | ((i * 17) << 8 & 0xff00)
                    | (i * 7 & 0xff);
        }
        for (int[] img : images) {
            for (int maxColors : new int[] {8, 32, 128}) {
                Map<Integer, Integer> res = QuantizerCelebi.quantize(img, maxColors);
                List<Map.Entry<Integer, Integer>> entries = new ArrayList<>(res.entrySet());
                entries.sort(Comparator.comparingLong(e -> Integer.toUnsignedLong(e.getKey())));
                List<String> parts = new ArrayList<>();
                for (Map.Entry<Integer, Integer> e : entries) {
                    parts.add(hex(e.getKey()) + ":" + e.getValue());
                }
                List<String> imgHex = new ArrayList<>();
                for (int px : img) {
                    imgHex.add(hex(px));
                }
                String imgKey = String.join(",", imgHex);
                out.add("quantize " + maxColors + " " + imgKey + " = " + String.join(" ", parts));
                List<String> scored = new ArrayList<>();
                for (int c : Score.score(res)) {
                    scored.add(hex(c));
                }
                out.add("score " + maxColors + " " + imgKey + " = " + String.join(" ", scored));
            }
        }

        System.out.println(String.join("\n", out));
    }
}
