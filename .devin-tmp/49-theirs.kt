/** `LayoutDirection.flip()` — private in AlertDialog.kt; mirrored for the
 * dialog flow row. */
private fun LayoutDirection.flipped(): LayoutDirection =
    when (this) {
        LayoutDirection.Ltr -> LayoutDirection.Rtl
        LayoutDirection.Rtl -> LayoutDirection.Ltr
    }

/** One dialog action button: a `TextButton` driven by the item's authored
 * state through its own `ReplayableInteractionSource`, traced as
 * `{tag}action{i}` with its label at `text:{textId}`. */
@Composable
private fun DialogActionButton(
    label: String,
    item: GroupItem,
    scene: Scene,
    tracer: Tracer,
    tag: String,
    textId: String,
    density: Float,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
) {
    val interactionSource = remember { ReplayableInteractionSource() }
    if (!item.disabled) {
        emitStateInteractions(
            item.state,
            scene,
            tracer,
            tag,
            interactionSource,
            emitPress,
            Offset(20f * density, 20f * density),
            density,
            null,
        )
    }
    val style = MaterialTheme.typography.labelLarge
    TextButton(
        onClick = {},
        enabled = !item.disabled,
        interactionSource = interactionSource,
        modifier = Modifier.track(tracer, tag),
    ) {
        Text(
            label,
            style = style,
            modifier = Modifier.trackText(tracer, textId, density),
            onTextLayout = recordTextLayout(
                tracer,
                textId,
                LocalDensity.current,
                androidx.compose.ui.platform.LocalFontFamilyResolver.current,
                style.fontFamily,
            ),
        )
    }
}

/** `AlertDialogContent` (AlertDialog.kt) rendered inline: the scrim
 * `Modal` paints when the popup opens, then the centered
 * `sizeIn(280..560)` pane. `Dialog` opens a platform window and
 * `Surface`'s `shadowElevation` deadlocks layoutlib, so the pane is
 * unelevated — the Slint side sets `cast_shadow: false`. `title` maps to
 * the title slot, `text` to the `text` slot, `items` to the actions
 * ([dismiss, …, confirm] in display order — the flipped `FlowRow` makes
 * the confirm first child), `icon` to the optional header icon.
 *
 * `basic-alert-dialog` shares the scrim and the `sizeIn` box but the
 * content is the caller's own — `SceneBasicDialogContent`. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun StateAlertDialog(
    widget: Widget,
    scene: Scene,
    tracer: Tracer,
    tag: String,
    density: Float,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
    textBase: Int,
) {
    val scheme = MaterialTheme.colorScheme
    val typography = MaterialTheme.typography
    // `Modal`'s scrim: the `scrim` role at `ScrimTokens.container_opacity`.
    Box(
        Modifier.fillMaxSize()
            .background(scheme.scrim.copy(alpha = 0.32f))
            .track(tracer, "scrim${tag.filter(Char::isDigit)}"),
    )
    if (widget.kind == "basic-alert-dialog") {
        SceneBasicDialogContent(widget, scene, tracer, tag, density, emitPress, textBase)
        return
    }
    var textIndex = textBase
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        // `BasicAlertDialog`'s `sizeIn(280, 560)` +
        // `propagateMinConstraints` — upstream internal constants
        // `DialogMinWidth`/`DialogMaxWidth`.
        Box(
            Modifier.sizeIn(minWidth = 280.dp, maxWidth = 560.dp),
            propagateMinConstraints = true,
        ) {
            Surface(
                modifier = Modifier.track(tracer, tag),
                shape = MaterialTheme.shapes.extraLarge,
                color = scheme.surfaceContainerHigh,
                tonalElevation = 0.dp,
                shadowElevation = 0.dp,
            ) {
                Column(Modifier.padding(24.dp)) {
                    widget.icon?.let { stem ->
                        CompositionLocalProvider(
                            LocalContentColor provides scheme.secondary,
                        ) {
                            Box(
                                Modifier.padding(bottom = 16.dp)
                                    .align(Alignment.CenterHorizontally),
                            ) {
                                Icon(
                                    sceneIcon(stem),
                                    contentDescription = null,
                                    modifier = Modifier.size(24.dp),
                                )
                            }
                        }
                    }
                    widget.title?.let { title ->
                        val textId = "text:${textIndex++}"
                        Text(
                            title,
                            style = typography.headlineSmall,
                            color = scheme.onSurface,
                            modifier =
                                Modifier.padding(bottom = 16.dp)
                                    .align(
                                        if (widget.icon != null) {
                                            Alignment.CenterHorizontally
                                        } else {
                                            Alignment.Start
                                        },
                                    )
                                    .trackText(tracer, textId, density),
                            onTextLayout = recordTextLayout(
                                tracer,
                                textId,
                                LocalDensity.current,
                                androidx.compose.ui.platform.LocalFontFamilyResolver.current,
                                typography.headlineSmall.fontFamily,
                            ),
                        )
                    }
                    widget.text?.let { text ->
                        val textId = "text:${textIndex++}"
                        Text(
                            text,
                            style = typography.bodyMedium,
                            color = scheme.onSurfaceVariant,
                            modifier =
                                Modifier.padding(bottom = 24.dp)
                                    .align(Alignment.Start)
                                    .trackText(tracer, textId, density),
                            onTextLayout = recordTextLayout(
                                tracer,
                                textId,
                                LocalDensity.current,
                                androidx.compose.ui.platform.LocalFontFamilyResolver.current,
                                typography.bodyMedium.fontFamily,
                            ),
                        )
                    }
                    Box(Modifier.align(Alignment.End)) {
                        // `AlertDialogFlowRow` verbatim: children
                        // [confirm, dismiss] in the flipped direction —
                        // confirm rightmost on one line, on top stacked.
                        val originalLayoutDirection = LocalLayoutDirection.current
                        CompositionLocalProvider(
                            LocalLayoutDirection provides
                                originalLayoutDirection.flipped(),
                        ) {
                            FlowRow(
                                horizontalArrangement =
                                    Arrangement.spacedBy(8.dp),
                                verticalArrangement =
                                    Arrangement.spacedBy(
                                        (
                                            8.dp -
                                                (
                                                    LocalMinimumInteractiveComponentSize
                                                        .current -
                                                        ButtonDefaults.MinHeight
                                                    )
                                            ).coerceIn(0.dp, 8.dp),
                                    ),
                            ) {
                                CompositionLocalProvider(
                                    LocalLayoutDirection provides
                                        originalLayoutDirection,
                                ) {
                                    widget.items.asReversed().forEachIndexed { i, item ->
                                        val textId = "text:${textIndex++}"
                                        DialogActionButton(
                                            label = item.text ?: "",
                                            item = item,
                                            scene = scene,
                                            tracer = tracer,
                                            tag = "${tag}action$i",
                                            textId = textId,
                                            density = density,
                                            emitPress = emitPress,
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/** `BasicAlertDialog` content: upstream hands `content` only the
 * `sizeIn(280..560)` box — the pane here is the caller's own, drawn like
 * the canonical sample (a surface-coloured extra-large pane with a
 * headline and an end-aligned action row). */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun SceneBasicDialogContent(
    widget: Widget,
    scene: Scene,
    tracer: Tracer,
    tag: String,
    density: Float,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
    textBase: Int,
) {
    val scheme = MaterialTheme.colorScheme
    var textIndex = textBase
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Box(
            Modifier.sizeIn(minWidth = 280.dp, maxWidth = 560.dp),
            propagateMinConstraints = true,
        ) {
            Surface(
                modifier = Modifier.track(tracer, tag),
                shape = MaterialTheme.shapes.extraLarge,
                color = scheme.surfaceContainerHigh,
                tonalElevation = 0.dp,
                shadowElevation = 0.dp,
            ) {
                Column(
                    Modifier.padding(24.dp),
                    verticalArrangement = Arrangement.spacedBy(24.dp),
                ) {
                    widget.title?.let { title ->
                        val textId = "text:${textIndex++}"
                        Text(
                            title,
                            style = MaterialTheme.typography.headlineSmall,
                            color = scheme.onSurface,
                            modifier = Modifier.trackText(tracer, textId, density),
                            onTextLayout = recordTextLayout(
                                tracer,
                                textId,
                                LocalDensity.current,
                                androidx.compose.ui.platform.LocalFontFamilyResolver.current,
                                MaterialTheme.typography.headlineSmall.fontFamily,
                            ),
                        )
                    }
                    Row(
                        Modifier.align(Alignment.End),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        widget.items.forEachIndexed { i, item ->
                            val textId = "text:${textIndex++}"
                            DialogActionButton(
                                label = item.text ?: "",
                                item = item,
                                scene = scene,
                                tracer = tracer,
                                tag = "${tag}action$i",
                                textId = textId,
                                density = density,
                                emitPress = emitPress,
                            )
                        }
                    }
                }
            }
        }
    }
}

/** `date-picker`/`date-range-picker` ISO `YYYY-MM-DD` (or `YYYY-MM` → day 1)
 * → start-of-day UTC millis, the `*Millis` the upstream state classes take. */
private fun isoMillis(iso: String): Long =
    java.time.LocalDate.parse(
        if (iso.count { it == '-' } == 1) "$iso-01" else iso
    ).atStartOfDay(java.time.ZoneOffset.UTC).toInstant().toEpochMilli()

/** `DatePickerDialog` content rendered inline — `DatePickerDialog_android.kt`:
 * a `BasicAlertDialog` `Surface` at `requiredWidth(360).heightIn(max = 568)`
 * (`DatePickerModalTokens.ContainerWidth`/`ContainerHeight`, both `internal`)
 * holding `Column(SpaceBetween) { weighted content; aligned-end buttons }`.
 * The buttons row is upstream's verbatim: an `AlertDialogFlowRow` under the
 * `DialogButtonsPadding` (`PaddingValues(end = 6, bottom = 8)`,
 * `DialogButtonsMainAxisSpacing` 8 / `DialogButtonsCrossAxisSpacing` 8) with
 * the confirm first so the flipped row lands it rightmost — each wrapped in
 * the focus-management `Box`es that have no layout effect. `Dialog` opens a
 * platform window and `Surface` shadows deadlock layoutlib, so the scene is
 * the inline equivalent (the scrim drawn behind) — like `StateAlertDialog`.
 *
 * No `trackText`/`recordTextLayout` inside: the picker's own labels live in
 * the private upstream composables the mirror can't reach, so text indices
 * would misalign — these scenes verify text by pixels alone. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun StateDatePickerDialog(
    widget: Widget,
    scene: Scene,
    tracer: Tracer,
    tag: String,
    density: Float,
    emitPress: java.util.concurrent.CopyOnWriteArrayList<Pair<Long, Runnable>>,
) {
    val scheme = MaterialTheme.colorScheme
    Box(
        Modifier.fillMaxSize()
            .background(scheme.scrim.copy(alpha = 0.32f))
            .track(tracer, "scrim${tag.filter(Char::isDigit)}"),
    )
    // `SelectableDates` — the slint side's `selectable_from`/`selectable_to`
    // contiguous window. An inclusive day range: the `to` bound covers that
    // whole day (millis < next midnight).
    val selectableDates = remember(widget) {
        object : androidx.compose.material3.SelectableDates {
            private val from = widget.selectableFrom?.let(::isoMillis) ?: Long.MIN_VALUE
            private val to = widget.selectableTo?.let(::isoMillis) ?: Long.MAX_VALUE
            private val toEnd =
                if (to > Long.MAX_VALUE - 86399999L) Long.MAX_VALUE else to + 86399999L
            override fun isSelectableDate(utcTimeMillis: Long): Boolean =
                utcTimeMillis >= from && utcTimeMillis <= toEnd
            override fun isSelectableYear(year: Int): Boolean {
                val lo =
                    widget.selectableFrom
                        ?.let { java.time.LocalDate.parse(it).year }
                        ?: Int.MIN_VALUE
                val hi =
                    widget.selectableTo
                        ?.let { java.time.LocalDate.parse(it).year }
                        ?: Int.MAX_VALUE
                return year in lo..hi
            }
        }
    }
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Surface(
            modifier =
                Modifier.requiredWidth(360.dp)
                    .heightIn(max = 568.dp)
                    .track(tracer, tag),
            shape = MaterialTheme.shapes.extraLarge,
            color = scheme.surfaceContainerHigh,
            tonalElevation = 0.dp,
            shadowElevation = 0.dp,
        ) {
            Column(verticalArrangement = Arrangement.SpaceBetween) {
                Box(Modifier.weight(1f, fill = false)) {
                    val initialDisplayMode =
                        if (widget.displayMode == "input") {
                            androidx.compose.material3.DisplayMode.Input
                        } else {
                            androidx.compose.material3.DisplayMode.Picker
                        }
                    if (widget.kind == "date-range-picker") {
                        val state =
                            androidx.compose.material3.rememberDateRangePickerState(
                                initialSelectedStartDateMillis =
                                    widget.selectedStart?.let(::isoMillis),
                                initialSelectedEndDateMillis =
                                    widget.selectedEnd?.let(::isoMillis),
                                initialDisplayedMonthMillis =
                                    widget.displayed?.let(::isoMillis),
                                yearRange = widget.yearMin..widget.yearMax,
                                initialDisplayMode = initialDisplayMode,
                                selectableDates = selectableDates,
                            )
                        androidx.compose.material3.DateRangePicker(
                            state = state,
                            showModeToggle = widget.showModeToggle,
                            // Upstream autofocuses the input field ~400ms in —
                            // the blinking cursor makes captures flaky.
                            focusRequester = null,
                        )
                    } else {
                        val state =
                            androidx.compose.material3.rememberDatePickerState(
                                initialSelectedDateMillis =
                                    widget.selectedDate?.let(::isoMillis),
                                initialDisplayedMonthMillis =
                                    widget.displayed?.let(::isoMillis),
                                yearRange = widget.yearMin..widget.yearMax,
                                initialDisplayMode = initialDisplayMode,
                                selectableDates = selectableDates,
                            )
                        androidx.compose.material3.DatePicker(
                            state = state,
                            title =
                                widget.title?.let { text ->
                                    {
                                        // Upstream's default title slot carries
                                        // `Modifier.padding(DatePickerTitlePadding)`
                                        // — start 24 / end 12 / top 16
                                        // (DatePicker.kt `DatePickerTitlePadding`,
                                        // private val).
                                        Text(
                                            text,
                                            modifier =
                                                Modifier.padding(
                                                    start = 24.dp,
                                                    end = 12.dp,
                                                    top = 16.dp,
                                                ),
                                        )
                                    }
                                }
                                    ?: {
                                        // Same `DatePickerTitlePadding` the upstream
                                        // `title` param default applies.
                                        androidx.compose.material3.DatePickerDefaults
                                            .DatePickerTitle(
                                                displayMode = state.displayMode,
                                                modifier =
                                                    Modifier.padding(
                                                        start = 24.dp,
                                                        end = 12.dp,
                                                        top = 16.dp,
                                                    ),
                                            )
                                    },
                            showModeToggle = widget.showModeToggle,
                            focusRequester = null,
                        )
                    }
                }
                Box(Modifier.align(Alignment.End).padding(end = 6.dp, bottom = 8.dp)) {
                    // `DialogTokens.ActionLabelTextColor`/`ActionLabelTextFont`
                    // — the stable `TextButton` overload the dialog samples
                    // and `emitStateInteractions`-driven `DialogActionButton`
                    // use (labelLarge/onSurface label colors).
                    val confirmInteraction = remember { ReplayableInteractionSource() }
                    val originalLayoutDirection = LocalLayoutDirection.current
                    CompositionLocalProvider(
                        LocalLayoutDirection provides originalLayoutDirection.flipped(),
                    ) {
                        FlowRow(
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                            verticalArrangement =
                                Arrangement.spacedBy(
                                    (
                                        8.dp -
                                            (
                                                LocalMinimumInteractiveComponentSize.current -
                                                    ButtonDefaults.MinHeight
                                                )
                                        ).coerceIn(0.dp, 8.dp),
                                ),
                        ) {
                            CompositionLocalProvider(
                                LocalLayoutDirection provides originalLayoutDirection,
                            ) {
                                // Children laid out [confirm, dismiss] — the
                                // flipped direction puts the confirm
                                // rightmost like the upstream dialog.
                                TextButton(
                                    onClick = {},
                                    enabled = widget.confirmEnabled,
                                    interactionSource = confirmInteraction,
                                    modifier = Modifier.track(tracer, "${tag}action0"),
                                ) {
                                    Text("OK", style = MaterialTheme.typography.labelLarge)
                                }
                                TextButton(
                                    onClick = {},
                                    modifier = Modifier.track(tracer, "${tag}action1"),
                                ) {
                                    Text("Cancel", style = MaterialTheme.typography.labelLarge)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}


