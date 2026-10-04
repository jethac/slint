/** `navigation-bar` (the 80dp tall bar) / `short-navigation-bar` (the 64dp
 * expressive bar) — `navbar{n}` elements. `nav_events`
 * ([["select", bar, item, ms], ...]) flips the bar's `selectedIndex` at the
 * same mock-clock beats the Slint case's letter-key dispatches fire on —
 * Paparazzi can't dispatch the pointer click either side needs. */
@Composable
private fun StateNavBar(
    widget: Widget,
    scene: Scene,
    tracer: Tracer,
    tag: String,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
) {
    val barOrdinal = tag.removePrefix("navbar").toInt()
    var selectedIndex by remember { mutableStateOf(widget.selectedIndex) }
    scene.params.optJSONArray("nav_events")?.let { events ->
        for (i in 0 until events.length()) {
            val ev = events.getJSONArray(i)
            if (ev.getString(0) != "select" || ev.getInt(1) != barOrdinal) continue
            val item = ev.getInt(2)
            val at = ev.getLong(3)
            DisposableEffect(tag, i) {
                val entry = at to Runnable { selectedIndex = item }
                emitPress.add(entry)
                onDispose { emitPress.remove(entry) }
            }
        }
    }
    val insets = WindowInsets(0, 0, 0, 0)
    val base = Modifier
        .offset(widget.x.dp, widget.y.dp)
        .then(if (widget.width > 0) Modifier.width(widget.width.dp) else Modifier)
        .track(tracer, tag)

    when (widget.kind) {
        "short-navigation-bar" -> ShortNavigationBar(
            modifier = base,
            arrangement = if (widget.navArrangement == "centered")
                ShortNavigationBarArrangement.Centered else ShortNavigationBarArrangement.EqualWeight,
            windowInsets = insets,
        ) {
            // alpha18's bar Layout measures `content` as a single measurable
            // under this harness (the whole lambda is one placeable), so the
            // arrangement policies — which divide the item children — see
            // one child spanning the bar. One Row child preserves the pinned
            // math: EqualWeight → weight(1f) cells filling the bar
            // (EqualWeightContentMeasurePolicy at the #3 pin); Centered →
            // widthIn(min..max) cells in a centered Row
            // (CenteredContentMeasurePolicy: padding =
            // ((100-10*(count+3))/2)% of the bar per side, item min width
            // (W-2*pad)/count, max W/count).
            val n = maxOf(widget.items.size, 1)
            val barW = widget.width
            Row(
                modifier = if (widget.navArrangement == "centered")
                    Modifier.fillMaxHeight().requiredWidth(barW.dp)
                else Modifier.fillMaxSize(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = if (widget.navArrangement == "centered")
                    Arrangement.Center else Arrangement.Start,
            ) {
                val pad = ((100f - 10f * (n + 3)) / 2f / 100f * barW).roundToInt()
                val itemMinW = ((barW - pad * 2) / n).toInt().dp
                val itemMaxW = (barW / n).toInt().dp
                widget.items.forEachIndexed { i, item ->
                    ShortNavigationBarItem(
                        selected = i == selectedIndex,
                        onClick = {},
                        icon = { NavItemIcon(item, i, selectedIndex) },
                        modifier = if (widget.navArrangement == "centered")
                            Modifier.widthIn(min = itemMinW, max = itemMaxW).fillMaxHeight()
                        else Modifier.weight(1f).fillMaxHeight(),
                        enabled = item.enabled,
                        label = item.text.takeIf { it.isNotEmpty() }?.let { { Text(it) } },
                        iconPosition = if (widget.iconPosition == "start")
                            NavigationItemIconPosition.Start else NavigationItemIconPosition.Top,
                    )
                }
            }
        }
        "navigation-bar" -> NavigationBar(
            modifier = base,
            windowInsets = insets,
        ) {
            widget.items.forEachIndexed { i, item ->
                NavigationBarItem(
                    selected = i == selectedIndex,
                    onClick = {},
                    icon = { NavItemIcon(item, i, selectedIndex) },
                    enabled = item.enabled,
                    label = item.text.takeIf { it.isNotEmpty() }?.let { { Text(it) } },
                    alwaysShowLabel = widget.alwaysShowLabel,
                )
            }
        }
        else -> error("unknown navigation-bar kind ${widget.kind}")
    }
}

/** The bar item's `icon` slot: the selected glyph when the item is
 * selected, wrapped in `BadgedBox` like the upstream samples. */
@Composable
private fun NavItemIcon(item: GroupItem, i: Int, selectedIndex: Int) {
    val stem = (if (i == selectedIndex) item.selectedIcon else item.icon) ?: item.icon ?: "check"
    if (item.badge != null) {
        BadgedBox(badge = { Badge { Text(item.badge) } }) {
            Icon(sceneIcon(stem), contentDescription = null)
        }
    } else {
        Icon(sceneIcon(stem), contentDescription = null)
    }
}
