---
title: MenuItem
description: MenuItem content
---

`MenuItem`

This structure represents a MenuItem with icons, text, supporting text, and selection state.

- **`text`** (_string_): The text to display in the item.
- **`icon`** (_image_): The icon to display in the item's leading slot.
- **`selected_icon`** (_image_): The icon shown in the leading slot while the item is selected or checked.
- **`trailing_icon`** (_image_): The icon to display in the item's trailing slot.
- **`trailing_text`** (_string_): The text to display in the item's trailing slot when no trailing icon is set.
- **`supporting_text`** (_string_): Additional text displayed under the item's label.
- **`selected`** (_bool_): Whether the item is selected.
- **`checked`** (_bool_): Whether the item is checked.
- **`disabled`** (_bool_): Whether the item is disabled. Defaults to `false` — items are enabled unless set.
