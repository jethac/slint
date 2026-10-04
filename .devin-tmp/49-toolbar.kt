/** Toolbar widgets (`horizontal-floating-toolbar`, `vertical-floating-toolbar`,
 * `flexible-bottom-app-bar`): geometry plus the props the Slint parity case
 * sets. Elements are named `appbar{n}` in scene order. Window insets are
 * zeroed — the Slint side has no inset concept. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun StateToolbar(
    widget: Widget,
    scene: Scene,
    tracer: Tracer,
    tag: String,
    density: Float,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
) {
    var modifier =
        Modifier.offset(widget.x.dp, widget.y.dp)
            .track(tracer, tag)
    when (widget.kind) {
        "flexible-bottom-app-bar" -> {
            val state = rememberBottomAppBarState(
                initialHeightOffsetLimit = -Float.MAX_VALUE,
                initialHeightOffset = widget.heightOffset,
                initialContentOffset = widget.contentOffset,
            )
            val arrangement =
                if (widget.arrangement == "spaced") {
                    Arrangement.spacedBy(
                        widget.spacing.dp,
                        androidx.compose.ui.Alignment.CenterHorizontally,
                    )
                } else {
                    Arrangement.SpaceBetween
                }
            FlexibleBottomAppBar(
                modifier = modifier.width(widget.width.dp),
                horizontalArrangement = arrangement,
                expandedHeight =
                    if (widget.expandedHeight > 0f) widget.expandedHeight.dp
                    else BottomAppBarDefaults.FlexibleBottomAppBarHeight,
                windowInsets = WindowInsets(0, 0, 0, 0),
                scrollBehavior = BottomAppBarDefaults.exitAlwaysScrollBehavior(state),
            ) {
                widget.icons.forEach { icon ->
                    IconButton(onClick = {}) {
                        Icon(sceneIcon(icon), contentDescription = null)
                    }
                }
            }
        }
        else -> {
            val colors =
                if (widget.colorStyle == "vibrant") {
                    FloatingToolbarDefaults.vibrantFloatingToolbarColors()
                } else {
                    FloatingToolbarDefaults.standardFloatingToolbarColors()
                }
            var expanded by remember { mutableStateOf(widget.expanded) }
            // A `press` action hit-testing this toolbar flips `expanded` —
            // the generated Slint case assigns its in-out `expanded` on the
            // same dispatch beat.
            val pressAction = scene.actions.firstOrNull { it.kind == "press" }
            if (pressAction != null) {
                val toggle = Runnable {
                    val b = tracer.elementBounds[tag]
                    val hits = b == null ||
                        (pressAction.x * density >= b.left &&
                            pressAction.x * density <= b.right &&
                            pressAction.y * density >= b.top &&
                            pressAction.y * density <= b.bottom)
                    if (hits) expanded = !expanded
                }
                DisposableEffect(toggle) {
                    val entry = pressAction.at to toggle
                    emitPress.add(entry)
                    onDispose { emitPress.remove(entry) }
                }
            }
            // The Slint side names its toolbar fab `fab`; `find_first`
            // resolves the trace's `fab` to the first fab-bearing toolbar in
            // scene order — track it here only, so multi-fab scenes stay
            // symmetric (scenes keep one fab each anyway: each untraced
            // fab's shadow would land in the strict layer).
            val firstFab = scene.widgets.indexOfFirst { it.fabIcon != null }
            val fabModifier =
                if (scene.widgets.indexOfFirst { it === widget } == firstFab) {
                    Modifier.track(tracer, "fab")
                } else {
                    Modifier
                }
            // The clipped toolbar surface isn't modifier-reachable, but
            // the pinned `*WithFabLayout` fixes it inside the constant
            // component bounds (the fab slot is 56dp + 8dp gap; the surface
            // is 64dp on the short axis, centered, and the collapse clips
            // it toward the fab-adjacent edge): derive it from the tracked
            // `appbar` rect and `expanded`. Only fab-bearing toolbars emit
            // `container`, the name the Slint component gives that surface.
            if (widget.fabIcon != null) {
                val g = 64 * density
                val i = 8 * density
                fun surfaceRect(b: Rect, exp: Boolean): Rect =
                    if (widget.kind == "horizontal-floating-toolbar") {
                        if (widget.fabPosition == "start") {
                            if (exp) {
                                Rect(b.left + g, b.top + i, b.right, b.top + g + i)
                            } else {
                                Rect(b.left + g, b.top + i, b.left + g, b.top + g + i)
                            }
                        } else {
                            if (exp) {
                                Rect(b.left, b.top + i, b.right - g, b.top + g + i)
                            } else {
                                Rect(b.right - g, b.top + i, b.right - g, b.top + g + i)
                            }
                        }
                    } else {
                        if (widget.fabPosition == "top") {
                            if (exp) {
                                Rect(b.left + i, b.top + g, b.left + g + i, b.bottom)
                            } else {
                                Rect(b.left + i, b.top + g, b.left + g + i, b.top + g)
                            }
                        } else {
                            if (exp) {
                                Rect(b.left + i, b.top, b.left + g + i, b.bottom - g)
                            } else {
                                Rect(b.left + i, b.bottom - g, b.left + g + i, b.bottom - g)
                            }
                        }
                    }
                // Eager emit for the pre-layout first frame: the layout
                // bounds are 80dp on the short axis and
                // content(40dp/icon) + 16dp padding + the 64dp fab slot on
                // the long; `SideEffect` keeps the rect fresh afterwards —
                // including the collapsed pin edge when `expanded` flips.
                val n =
                    widget.icons.size +
                        widget.leadingIcons.size +
                        widget.trailingIcons.size
                val (cw, ch) =
                    if (widget.kind == "horizontal-floating-toolbar") {
                        (80 + 40 * n) * density to 80 * density
                    } else {
                        80 * density to (80 + 40 * n) * density
                    }
                tracer.elementBounds["container"] =
                    surfaceRect(
                        Rect(
                            widget.x * density,
                            widget.y * density,
                            widget.x * density + cw,
                            widget.y * density + ch,
                        ),
                        expanded,
                    )
                SideEffect {
                    tracer.elementBounds[tag]?.let {
                        tracer.elementBounds["container"] = surfaceRect(it, expanded)
                    }
                }
            }
            val fab: (@Composable () -> Unit)? = widget.fabIcon?.let { icon ->
                {
                    if (widget.colorStyle == "vibrant") {
                        FloatingToolbarDefaults.VibrantFloatingActionButton(
                            onClick = {},
                            modifier = fabModifier,
                        ) {
                            Icon(sceneIcon(icon), contentDescription = null)
                        }
                    } else {
                        FloatingToolbarDefaults.StandardFloatingActionButton(
                            onClick = {},
                            modifier = fabModifier,
                        ) {
                            Icon(sceneIcon(icon), contentDescription = null)
                        }
                    }
                }
            }
            if (widget.kind == "horizontal-floating-toolbar") {
                val leading: (@Composable androidx.compose.foundation.layout.RowScope.() -> Unit)? =
                    widget.leadingIcons.takeIf { it.isNotEmpty() }?.let { icons ->
                        {
                            icons.forEach { icon ->
                                IconButton(onClick = {}) {
                                    Icon(sceneIcon(icon), contentDescription = null)
                                }
                            }
                        }
                    }
                val trailing: (@Composable androidx.compose.foundation.layout.RowScope.() -> Unit)? =
                    widget.trailingIcons.takeIf { it.isNotEmpty() }?.let { icons ->
                        {
                            icons.forEach { icon ->
                                IconButton(onClick = {}) {
                                    Icon(sceneIcon(icon), contentDescription = null)
                                }
                            }
                        }
                    }
                if (fab != null) {
                    HorizontalFloatingToolbar(
                        expanded = expanded,
                        floatingActionButton = fab,
                        modifier = modifier,
                        colors = colors,
                        floatingActionButtonPosition =
                            if (widget.fabPosition == "start") {
                                FloatingToolbarHorizontalFabPosition.Start
                            } else {
                                FloatingToolbarHorizontalFabPosition.End
                            },
                    ) {
                        widget.icons.forEach { icon ->
                            IconButton(onClick = {}) {
                                Icon(sceneIcon(icon), contentDescription = null)
                            }
                        }
                    }
                } else {
                    HorizontalFloatingToolbar(
                        expanded = expanded,
                        modifier = modifier,
                        colors = colors,
                        leadingContent = leading,
                        trailingContent = trailing,
                    ) {
                        widget.icons.forEach { icon ->
                            IconButton(onClick = {}) {
                                Icon(sceneIcon(icon), contentDescription = null)
                            }
                        }
                    }
                }
            } else {
                val leading: (@Composable androidx.compose.foundation.layout.ColumnScope.() -> Unit)? =
                    widget.leadingIcons.takeIf { it.isNotEmpty() }?.let { icons ->
                        {
                            icons.forEach { icon ->
                                IconButton(onClick = {}) {
                                    Icon(sceneIcon(icon), contentDescription = null)
                                }
                            }
                        }
                    }
                val trailing: (@Composable androidx.compose.foundation.layout.ColumnScope.() -> Unit)? =
                    widget.trailingIcons.takeIf { it.isNotEmpty() }?.let { icons ->
                        {
                            icons.forEach { icon ->
                                IconButton(onClick = {}) {
                                    Icon(sceneIcon(icon), contentDescription = null)
                                }
                            }
                        }
                    }
                if (fab != null) {
                    VerticalFloatingToolbar(
                        expanded = expanded,
                        floatingActionButton = fab,
                        modifier = modifier,
                        colors = colors,
                        floatingActionButtonPosition =
                            if (widget.fabPosition == "top") {
                                FloatingToolbarVerticalFabPosition.Top
                            } else {
                                FloatingToolbarVerticalFabPosition.Bottom
                            },
                    ) {
                        widget.icons.forEach { icon ->
                            IconButton(onClick = {}) {
                                Icon(sceneIcon(icon), contentDescription = null)
                            }
                        }
                    }
                } else {
                    VerticalFloatingToolbar(
                        expanded = expanded,
                        modifier = modifier,
                        colors = colors,
                        leadingContent = leading,
                        trailingContent = trailing,
                    ) {
                        widget.icons.forEach { icon ->
                            IconButton(onClick = {}) {
                                Icon(sceneIcon(icon), contentDescription = null)
                            }
                        }
                    }
                }
            }
        }
    }
}
