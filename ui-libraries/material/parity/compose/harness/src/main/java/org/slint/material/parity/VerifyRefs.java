// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

package org.slint.material.parity;

import java.awt.image.BufferedImage;
import java.io.File;
import javax.imageio.ImageIO;

/** Pixel comparison for the `Verify committed Compose references` CI step.
 * Re-recording a scene does not reproduce PNG bytes exactly: layoutlib's
 * antialiasing phase jitters a handful of container-edge pixels by ±1 level
 * between runs, so a byte-level `git diff` flags noise as drift. This
 * comparer absorbs that noise floor — a few hundredths of a percent of
 * pixels at a couple of levels — while still failing any real drift (a
 * missed morph or a moved element changes hundreds of pixels by tens of
 * levels).
 *
 * `VerifyRefs <committed.png> <recorded.png>` exits 0 when the recorded
 * frame is within tolerance of the committed reference, 1 otherwise. */
public final class VerifyRefs {
    /** A pixel differing by more than this many levels in any channel is
     * real drift, no matter how isolated the pixel is. */
    private static final int LEVEL_EPS = 8;
    /** At most this fraction of pixels may differ at all (any channel
     * delta > 0) — the observed noise floor is ~8 edge pixels in ~20k. */
    private static final double ANY_DELTA_FRACTION = 0.002;
    /** Absolute floor for tiny frames: pixels allowed to differ at all. */
    private static final int ANY_DELTA_MIN = 32;

    private VerifyRefs() {}

    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            System.err.println("usage: VerifyRefs <committed.png> <recorded.png>");
            System.exit(2);
        }
        File committedFile = new File(args[0]);
        File recordedFile = new File(args[1]);
        if (!committedFile.isFile()) {
            System.err.println(committedFile + ": no committed reference");
            System.exit(1);
        }
        if (!recordedFile.isFile()) {
            System.err.println(recordedFile + ": recorded frame missing — scene produced no output");
            System.exit(1);
        }
        BufferedImage committed = ImageIO.read(committedFile);
        BufferedImage recorded = ImageIO.read(recordedFile);
        if (committed.getWidth() != recorded.getWidth()
                || committed.getHeight() != recorded.getHeight()) {
            System.err.printf(
                    "%s: dimensions drifted %dx%d -> %dx%d%n",
                    recordedFile,
                    committed.getWidth(),
                    committed.getHeight(),
                    recorded.getWidth(),
                    recorded.getHeight());
            System.exit(1);
        }
        int w = committed.getWidth();
        int h = committed.getHeight();
        int anyDelta = 0;
        int maxDelta = 0;
        int worstX = -1;
        int worstY = -1;
        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                int a = committed.getRGB(x, y);
                int b = recorded.getRGB(x, y);
                if (a == b) {
                    continue;
                }
                int delta = 0;
                for (int shift = 0; shift < 32; shift += 8) {
                    delta = Math.max(delta, Math.abs(((a >>> shift) & 0xff) - ((b >>> shift) & 0xff)));
                }
                anyDelta++;
                if (delta > maxDelta) {
                    maxDelta = delta;
                    worstX = x;
                    worstY = y;
                }
            }
        }
        int allowed = Math.max(ANY_DELTA_MIN, (int) (w * h * ANY_DELTA_FRACTION));
        if (maxDelta > LEVEL_EPS || anyDelta > allowed) {
            System.err.printf(
                    "%s: %d pixels differ (allowed %d), worst delta %d at (%d,%d)%n",
                    recordedFile, anyDelta, allowed, maxDelta, worstX, worstY);
            System.exit(1);
        }
        if (anyDelta > 0) {
            System.out.printf(
                    "%s: %d pixels differ within the noise floor (max delta %d)%n",
                    recordedFile, anyDelta, maxDelta);
        }
        System.exit(0);
    }
}
