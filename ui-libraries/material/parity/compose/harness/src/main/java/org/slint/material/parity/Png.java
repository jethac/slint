// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

package org.slint.material.parity;

import java.awt.Color;
import java.awt.Graphics2D;
import java.awt.image.BufferedImage;
import java.io.File;
import java.io.IOException;
import javax.imageio.ImageIO;
import org.json.JSONException;
import org.json.JSONObject;

/** PNG writing and text-mask bitmaps for the parity harness. These live in a
 * Java source because AGP compiles Kotlin test sources against the whittled
 * JDK image (`androidJdkImage`), which drops `java.desktop`; `javac` still
 * gets the real JDK. */
public final class Png {
    private Png() {}

    public static void write(File path, BufferedImage image) throws IOException {
        File parent = path.getParentFile();
        if (parent != null) {
            parent.mkdirs();
        }
        ImageIO.write(image, "PNG", path);
    }

    /** Builds the text mask for a frame and writes it as PNG. */
    public static void writeMask(File path, JSONObject text, float density, int widthPx, int heightPx)
            throws IOException, JSONException {
        write(path, mask(text, density, widthPx, heightPx));
    }

    /** White where the frame's `text:` metrics sit (rounded outward plus
     * 1 px for antialiased glyph edges), black elsewhere; same dimensions as
     * the frame — the comparator indexes it by pixel position. */
    public static BufferedImage mask(JSONObject text, float density, int widthPx, int heightPx)
            throws JSONException {
        BufferedImage bmp = new BufferedImage(widthPx, heightPx, BufferedImage.TYPE_INT_ARGB);
        Graphics2D g = bmp.createGraphics();
        g.setColor(Color.BLACK);
        g.fillRect(0, 0, widthPx, heightPx);
        g.setColor(Color.WHITE);
        for (String id : text.keySet()) {
            JSONObject m = text.getJSONObject(id);
            double x = m.getDouble("x") * density;
            double y = m.getDouble("y") * density;
            double r = m.getDouble("w") * density;
            double b = m.getDouble("h") * density;
            int x0 = (int) Math.floor(x) - 1;
            int y0 = (int) Math.floor(y) - 1;
            int x1 = (int) Math.ceil(x + r) + 1;
            int y1 = (int) Math.ceil(y + b) + 1;
            g.fillRect(x0, y0, Math.max(0, x1 - x0), Math.max(0, y1 - y0));
        }
        g.dispose();
        return bmp;
    }
}
