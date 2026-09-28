<!--
Copyright © SixtyFPS GmbH <info@slint.dev>
SPDX-License-Identifier: MIT
-->

# Google Sans Flex subset for MCU

`GoogleSansFlex-subset.ttf` (~1.6 MB) is a glyph-subset copy of the Material
library's bundled `GoogleSansFlex[GRAD,ROND,opsz,slnt,wdth,wght].ttf`
(~4.2 MB). MCU flash can't hold the full font, so this example registers the
subset under the original family name instead of importing `material.slint`'s
full-font registration. All six variable axes are kept; the glyph set is
reduced to Basic Latin (U+0020–U+007E).

Regenerate with fontTools:

```sh
pyftsubset \
    "ui-libraries/material/src/fonts/google-sans-flex/GoogleSansFlex[GRAD,ROND,opsz,slnt,wdth,wght].ttf" \
    --unicodes="U+0020-007E" \
    --name-IDs='*' --name-legacy --name-languages='*' \
    --output-file="ui-libraries/material/examples/mcu/fonts/GoogleSansFlex-subset.ttf"
```

The font is licensed under the SIL Open Font License 1.1 with no Reserved Font
Names — see `../../../src/fonts/google-sans-flex/OFL.txt` and
`TRADEMARKS.md`. The name table is otherwise unchanged, so the subset resolves
under the family name "Google Sans Flex" exactly like the full font. Text
outside the ASCII range renders with fallback glyphs, which is acceptable for
an example; a product should subset to the codepoints its UI actually ships.
