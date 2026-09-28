// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

package org.slint.material.parity;

import app.cash.paparazzi.Snapshot;
import app.cash.paparazzi.SnapshotHandler;
import java.awt.image.BufferedImage;
import java.io.File;
import java.io.IOException;
import java.util.function.BiConsumer;
import org.jetbrains.annotations.NotNull;

/** Routes each rendered frame to the writer the current scene installs —
 * `frame_<index>ms.png` for timestamps the scene asks for (motion) or the
 * last frame of the settle run (static). Lives in Java: Kotlin test sources
 * compile without `java.desktop`, so `BufferedImage` cannot be named there —
 * everything crosses the boundary as `Object`. */
public final class FrameSink implements SnapshotHandler {
    private BiConsumer<Integer, Object> sink;
    private Runnable onFramesDone;
    private BufferedImage lastImage;

    public void setSink(BiConsumer<Integer, Object> sink) {
        this.sink = sink;
    }

    public void setOnFramesDone(Runnable onFramesDone) {
        this.onFramesDone = onFramesDone;
    }

    /** The most recent frame handled, written as PNG (static settle frame). */
    public void writeLast(File path) throws IOException {
        if (lastImage != null) {
            Png.write(path, lastImage);
        }
    }

    /** Writes a frame handed over as Object — Java-side cast. */
    public static void writeFrame(File path, Object image) throws IOException {
        Png.write(path, (BufferedImage) image);
    }

    @NotNull
    @Override
    public FrameHandler newFrameHandler(@NotNull Snapshot snapshot, int frameCount, int fps) {
        return new FrameHandler() {
            private int index = 0;

            @Override
            public void handle(@NotNull BufferedImage image) {
                lastImage = image;
                if (sink != null) {
                    sink.accept(index++, image);
                }
            }

            @Override
            public void close() {
                if (onFramesDone != null) {
                    onFramesDone.run();
                }
            }
        };
    }

    @Override
    public void close() {}
}
