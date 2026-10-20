package com.cougot.prism.ui

import android.content.res.Configuration
import android.os.SystemClock
import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.pointer.PointerEvent
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.PlayerView
import com.cougot.prism.MainActivity
import com.cougot.prism.PlayerController
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlin.math.abs
import kotlin.math.ln
import kotlin.math.min
import kotlin.math.pow

private const val HOLD_DELAY_MS = 300L
private val ROTATION_LABELS = listOf("Auto", "Landscape", "Portrait")

// ---------- speed mapping: 0.25x … 4x on a log2 scale, 1x in the middle ----------

private val SPEED_SNAPS = floatArrayOf(0.25f, 0.5f, 0.75f, 1f, 1.25f, 1.5f, 2f, 3f, 4f)

private fun speedToPos(s: Float): Float = (((ln(s.toDouble()) / ln(2.0)) + 2.0) / 4.0).toFloat().coerceIn(0f, 1f)

private fun posToSpeed(p: Float): Float {
    val s = 2.0.pow(p * 4.0 - 2.0).toFloat()
    for (snap in SPEED_SNAPS) if (abs(s - snap) / snap < 0.04f) return snap
    return (s * 100f).toInt() / 100f
}

// ---------- small building blocks ----------

@Composable
fun IconBtn(
    icon: ImageVector,
    desc: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    size: Dp = 44.dp,
    bg: Color = Color.White.copy(alpha = 0.10f),
    tint: Color = P.Text,
    shape: Shape = CircleShape,
    iconSize: Dp = 20.dp,
    borderColor: Color? = null,
) {
    var m = modifier.size(size).clip(shape).background(bg)
    if (borderColor != null) m = m.border(1.dp, borderColor, shape)
    Box(
        m.clickable(role = Role.Button, onClick = onClick).semantics { contentDescription = desc },
        contentAlignment = Alignment.Center,
    ) { Icon(icon, null, tint = tint, modifier = Modifier.size(iconSize)) }
}

@Composable
private fun Chip(
    label: String,
    icon: ImageVector?,
    onClick: () -> Unit,
    active: Boolean = false,
    desc: String = label,
    modifier: Modifier = Modifier,
    bg: Color = P.Ink.copy(alpha = 0.5f),
) {
    val shape = RoundedCornerShape(22.dp)
    val base = modifier.height(44.dp).clip(shape)
    val bgMod = if (active) base.background(Brush.horizontalGradient(listOf(P.Coral, P.Yellow))) else base.background(bg)
    Row(
        bgMod.clickable(role = Role.Button, onClick = onClick).semantics { contentDescription = desc }
            .padding(horizontal = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        val fg = if (active) P.Ink else P.Text
        if (icon != null) Icon(icon, null, tint = fg, modifier = Modifier.size(20.dp))
        Text(label, color = fg, fontFamily = Grotesk, fontSize = 13.sp, fontWeight = if (active) FontWeight.SemiBold else FontWeight.Normal)
    }
}

@Composable
private fun MarkerButton(label: String, color: Color, desc: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val shape = RoundedCornerShape(12.dp)
    Box(
        modifier.height(44.dp).widthIn(min = 52.dp).clip(shape).background(color.copy(alpha = 0.14f))
            .border(1.dp, color, shape)
            .clickable(role = Role.Button, onClick = onClick).semantics { contentDescription = desc }
            .padding(horizontal = 12.dp),
        contentAlignment = Alignment.Center,
    ) { Text(label, color = P.Text, fontFamily = Grotesk, fontWeight = FontWeight.SemiBold, fontSize = 14.sp) }
}

/** Custom slider. [value] 0..1. Speed style: gradient track + ring thumb. Volume style: grey track + filled part. */
@Composable
private fun GradSlider(
    value: Float,
    onChange: (Float) -> Unit,
    desc: String,
    modifier: Modifier = Modifier,
    speedStyle: Boolean = false,
    dimmed: Boolean = false,
    onInteract: () -> Unit = {},
) {
    val change by rememberUpdatedState(onChange)
    val interact by rememberUpdatedState(onInteract)
    Canvas(
        modifier.height(44.dp)
            .semantics { contentDescription = desc }
            .pointerInput(Unit) {
                awaitEachGesture {
                    val down = awaitFirstDown()
                    val pad = 10.dp.toPx()
                    val w = (size.width - pad * 2).coerceAtLeast(1f)
                    fun at(x: Float) = ((x - pad) / w).coerceIn(0f, 1f)
                    change(at(down.position.x)); interact()
                    down.consume()
                    while (true) {
                        val ev = awaitPointerEvent()
                        val ch = ev.changes.firstOrNull { it.id == down.id } ?: break
                        if (!ch.pressed) break
                        change(at(ch.position.x)); interact()
                        ch.consume()
                    }
                }
            }
    ) {
        val pad = 10.dp.toPx()
        val w = size.width - pad * 2
        val cy = size.height / 2
        val th = 4.dp.toPx()
        val x = pad + w * value
        if (speedStyle) {
            drawRoundRect(
                Brush.horizontalGradient(listOf(P.Teal, P.Yellow, P.Red), startX = pad, endX = pad + w),
                topLeft = Offset(pad, cy - th / 2), size = Size(w, th), cornerRadius = CornerRadius(th / 2),
            )
            // tick at 1x
            drawRect(P.Text.copy(alpha = 0.6f), topLeft = Offset(pad + w * 0.5f - 0.5.dp.toPx(), cy + 7.dp.toPx()), size = Size(1.dp.toPx(), 5.dp.toPx()))
            drawCircle(P.Ink, radius = 10.dp.toPx(), center = Offset(x, cy))
            drawCircle(P.Yellow, radius = 8.5.dp.toPx(), center = Offset(x, cy), style = Stroke(3.dp.toPx()))
        } else {
            val fill = if (dimmed) P.Text.copy(alpha = 0.35f) else P.Text
            drawRoundRect(P.Text.copy(alpha = 0.18f), topLeft = Offset(pad, cy - th / 2), size = Size(w, th), cornerRadius = CornerRadius(th / 2))
            drawRoundRect(fill, topLeft = Offset(pad, cy - th / 2), size = Size(w * value, th), cornerRadius = CornerRadius(th / 2))
            drawCircle(if (dimmed) P.Muted else P.Text, radius = 7.dp.toPx(), center = Offset(x, cy))
        }
    }
}

/** Timeline with buffered range, damaged ranges (hatched), A/B loop section, and a draggable playhead. */
@Composable
private fun Timeline(ctrl: PlayerController, modifier: Modifier = Modifier, onInteract: () -> Unit = {}) {
    val interact by rememberUpdatedState(onInteract)
    val dur = ctrl.duration
    val pos = ctrl.position
    val buf = ctrl.buffered
    val a = ctrl.markA
    val b = ctrl.markB
    val dmg = ctrl.damaged.toList()
    val scrubbing = ctrl.scrubbing
    Canvas(
        modifier.fillMaxWidth().height(40.dp)
            .semantics { contentDescription = "Timeline, ${fmtTime(pos, false)} of ${fmtTime(dur, false)}" }
            .pointerInput(Unit) {
                awaitEachGesture {
                    val down = awaitFirstDown()
                    val d = ctrl.duration
                    if (d <= 0L) return@awaitEachGesture
                    val pad = 10.dp.toPx()
                    val w = (size.width - pad * 2).coerceAtLeast(1f)
                    fun at(x: Float): Long = (((x - pad) / w).coerceIn(0f, 1f) * d).toLong()
                    ctrl.beginScrub(); interact()
                    ctrl.scrubTo(at(down.position.x))
                    down.consume()
                    try {
                        while (true) {
                            val ev = awaitPointerEvent()
                            val ch = ev.changes.firstOrNull { it.id == down.id } ?: break
                            if (!ch.pressed) break
                            ctrl.scrubTo(at(ch.position.x)); interact()
                            ch.consume()
                        }
                    } finally {
                        ctrl.endScrub()
                    }
                }
            }
    ) {
        val pad = 10.dp.toPx()
        val w = size.width - pad * 2
        val cy = size.height / 2
        val th = 4.dp.toPx()
        fun xOf(ms: Long): Float = pad + if (dur > 0) (ms.toFloat() / dur).coerceIn(0f, 1f) * w else 0f
        val cr = CornerRadius(th / 2)
        drawRoundRect(P.Text.copy(alpha = 0.18f), Offset(pad, cy - th / 2), Size(w, th), cr)
        drawRoundRect(P.Text.copy(alpha = 0.32f), Offset(pad, cy - th / 2), Size((xOf(buf) - pad).coerceAtLeast(0f), th), cr)
        val px = xOf(pos)
        drawRoundRect(
            Brush.horizontalGradient(listOf(P.Red, P.Yellow), startX = pad, endX = maxOf(px, pad + 1f)),
            Offset(pad, cy - th / 2), Size((px - pad).coerceAtLeast(0f), th), cr,
        )
        // damaged ranges, hatched
        val hh = 12.dp.toPx()
        for (r in dmg) {
            val x0 = xOf(r.first)
            val x1 = maxOf(xOf(r.last), x0 + 3.dp.toPx())
            clipRect(x0, cy - hh / 2, x1, cy + hh / 2) {
                drawRect(P.Ink, Offset(x0, cy - hh / 2), Size(x1 - x0, hh))
                var x = x0 - hh
                while (x < x1) {
                    drawLine(P.Coral, Offset(x, cy + hh / 2), Offset(x + hh, cy - hh / 2), strokeWidth = 2.dp.toPx())
                    x += 4.dp.toPx()
                }
            }
        }
        // loop section
        if (a != null || b != null) {
            val xa = xOf(a ?: 0L)
            val xb = xOf(b ?: dur)
            val sh = 10.dp.toPx()
            if (xb > xa) {
                drawRoundRect(
                    Brush.horizontalGradient(listOf(P.Red, P.Yellow, P.Teal), startX = xa, endX = xb),
                    Offset(xa, cy - sh / 2), Size(xb - xa, sh), CornerRadius(3.dp.toPx()),
                )
            }
            val lw = 2.dp.toPx()
            val flag = Size(10.dp.toPx(), 8.dp.toPx())
            if (a != null) {
                drawRect(P.Red, Offset(xa - lw / 2, 0f), Size(lw, size.height))
                drawRoundRect(P.Red, Offset(xa - flag.width / 2, 0f), flag, CornerRadius(2.dp.toPx()))
            }
            if (b != null) {
                drawRect(P.Teal, Offset(xb - lw / 2, 0f), Size(lw, size.height))
                drawRoundRect(P.Teal, Offset(xb - flag.width / 2, 0f), flag, CornerRadius(2.dp.toPx()))
            }
        }
        if (dur > 0) {
            if (scrubbing) drawCircle(P.Yellow.copy(alpha = 0.35f), radius = 16.dp.toPx(), center = Offset(px, cy))
            drawCircle(P.Text, radius = 10.dp.toPx(), center = Offset(px, cy))
        }
    }
}

@Composable
private fun TimesRow(ctrl: PlayerController, color2: Color) {
    val a = ctrl.markA
    val b = ctrl.markB
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(fmtTime(ctrl.position), color = P.Text, fontFamily = Mono, fontSize = 13.sp)
        Text(
            when {
                a != null && b != null -> "A ${fmtTime(a)} – B ${fmtTime(b)} · ${"%.1f".format((b - a) / 1000f)}s"
                a != null -> "A ${fmtTime(a)} – end"
                else -> ""
            },
            color = P.Yellow, fontFamily = Mono, fontSize = 13.sp, textAlign = TextAlign.Center,
            maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f).padding(horizontal = 8.dp),
        )
        Text(fmtTime(ctrl.duration), color = color2, fontFamily = Mono, fontSize = 13.sp)
    }
}

// ---------- the video surface and gestures ----------

@Composable
private fun VideoSurface(ctrl: PlayerController) {
    AndroidView(
        factory = { c ->
            PlayerView(c).apply {
                useController = false
                resizeMode = AspectRatioFrameLayout.RESIZE_MODE_FIT
                setShutterBackgroundColor(android.graphics.Color.BLACK)
                setKeepContentOnPlayerReset(true)
                keepScreenOn = true
                player = ctrl.player
            }
        },
        modifier = Modifier.fillMaxSize(),
        onRelease = { it.player = null },
    )
}

/**
 * Tap: [onTap]. Horizontal drag: relative scrub. Press & hold in the left/right 30%: ramping seek.
 */
@Composable
private fun GestureLayer(ctrl: PlayerController, onTap: () -> Unit) {
    val tap by rememberUpdatedState(onTap)
    Box(
        Modifier.fillMaxSize().pointerInput(Unit) {
            awaitEachGesture {
                val down = awaitFirstDown(requireUnconsumed = false)
                val w = size.width.toFloat()
                val zone = when {
                    down.position.x < w * 0.3f -> -1
                    down.position.x > w * 0.7f -> 1
                    else -> 0
                }
                val slop = viewConfiguration.touchSlop
                val startPos = ctrl.position
                val startX = down.position.x
                val startY = down.position.y
                var originX = startX
                var mode = 0 // 0 undecided, 1 scrub, 2 hold, 3 ignore
                var completed = false
                try {
                    while (true) {
                        val event: PointerEvent? = if (mode == 0 && zone != 0) {
                            val remaining = HOLD_DELAY_MS - (SystemClock.uptimeMillis() - down.uptimeMillis)
                            if (remaining <= 0L) null else withTimeoutOrNull(remaining) { awaitPointerEvent() }
                        } else {
                            awaitPointerEvent()
                        }
                        if (event == null) {
                            mode = 2
                            ctrl.startHold(zone)
                            continue
                        }
                        val ch = event.changes.firstOrNull { it.id == down.id } ?: break
                        if (!ch.pressed) {
                            completed = true
                            break
                        }
                        val dx = ch.position.x - startX
                        val dy = ch.position.y - startY
                        if (mode == 0 && (abs(dx) > slop || abs(dy) > slop)) {
                            if (abs(dx) > abs(dy) && ctrl.duration > 0) {
                                mode = 1
                                originX = ch.position.x
                                ctrl.beginScrub()
                            } else {
                                mode = 3
                            }
                        }
                        if (mode == 1) {
                            val span = min(ctrl.duration, 120_000L).toFloat()
                            ctrl.scrubTo(startPos + ((ch.position.x - originX) / w * span).toLong())
                            ch.consume()
                        } else if (mode == 2) {
                            ch.consume()
                        }
                    }
                } finally {
                    if (mode == 1) ctrl.endScrub()
                    if (mode == 2) ctrl.endHold()
                }
                if (completed && mode == 0) tap()
            }
        }
    )
}

@Composable
private fun BoxScope.HoldHud(ctrl: PlayerController) {
    val dir = ctrl.holdDir
    if (dir == 0) return
    val fwd = dir > 0
    val glow = if (fwd) P.Teal else P.Red
    Box(
        Modifier.align(if (fwd) Alignment.CenterEnd else Alignment.CenterStart).fillMaxHeight().width(220.dp)
            .background(
                Brush.horizontalGradient(
                    if (fwd) listOf(Color.Transparent, glow.copy(alpha = 0.35f)) else listOf(glow.copy(alpha = 0.35f), Color.Transparent)
                )
            )
    )
    Column(
        Modifier.align(if (fwd) Alignment.CenterEnd else Alignment.CenterStart).padding(horizontal = 40.dp),
        horizontalAlignment = if (fwd) Alignment.End else Alignment.Start,
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        val rate = PlayerController.RATES[ctrl.holdLevel]
        Row(
            Modifier.clip(RoundedCornerShape(28.dp)).background(P.Ink.copy(alpha = 0.72f)).padding(horizontal = 18.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Icon(if (fwd) Ic.Forward else Ic.Rewind, null, tint = glow, modifier = Modifier.size(30.dp))
            Text("$rate×", color = P.Text, fontFamily = Mono, fontWeight = FontWeight.Bold, fontSize = 30.sp)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            for (i in PlayerController.RATES.indices) {
                Box(
                    Modifier.width(18.dp).height(6.dp).clip(RoundedCornerShape(3.dp))
                        .background(if (i <= ctrl.holdLevel) glow else P.Text.copy(alpha = 0.25f))
                )
            }
        }
        val delta = ctrl.holdDelta
        Row(
            Modifier.clip(RoundedCornerShape(8.dp)).background(P.Ink.copy(alpha = 0.72f)).padding(horizontal = 10.dp, vertical = 4.dp)
        ) {
            Text(fmtTime(ctrl.position) + "  ", color = P.Text, fontFamily = Mono, fontSize = 15.sp)
            Text(
                (if (delta >= 0) "+" else "−") + "%.1fs".format(abs(delta) / 1000f),
                color = if (fwd) P.Mint else P.Coral, fontFamily = Mono, fontSize = 15.sp,
            )
        }
    }
}

@Composable
private fun NoticeToast(ctrl: PlayerController, modifier: Modifier) {
    val msg = ctrl.notice
    val id = ctrl.noticeId
    LaunchedEffect(id) {
        delay(4500)
        if (ctrl.noticeId == id) ctrl.dismissNotice()
    }
    if (msg == null) return
    val warn = ctrl.noticeWarn
    Row(
        modifier.widthIn(max = 520.dp).clip(RoundedCornerShape(12.dp)).background(P.Ink.copy(alpha = 0.88f))
            .border(1.dp, if (warn) P.Coral.copy(alpha = 0.7f) else P.Yellow.copy(alpha = 0.5f), RoundedCornerShape(12.dp))
            .clickable { ctrl.dismissNotice() }
            .padding(horizontal = 12.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        if (warn) Icon(Ic.Warning, null, tint = P.Coral, modifier = Modifier.size(20.dp))
        Text(msg, color = P.Text, fontFamily = Grotesk, fontSize = 13.sp, lineHeight = 18.sp)
    }
}

@Composable
private fun BoxScope.FatalPanel(act: MainActivity) {
    val ctrl = act.ctrl
    val msg = ctrl.fatal ?: return
    Box(Modifier.matchParentSize().background(P.Ink.copy(alpha = 0.85f)))
    Column(
        Modifier.align(Alignment.Center).padding(24.dp).widthIn(max = 420.dp)
            .clip(RoundedCornerShape(18.dp)).background(P.Surface).border(1.dp, P.Coral.copy(alpha = 0.6f), RoundedCornerShape(18.dp))
            .padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Icon(Ic.Warning, null, tint = P.Coral, modifier = Modifier.size(24.dp))
            Text("Can't play this part", color = P.Text, fontFamily = Grotesk, fontWeight = FontWeight.Bold, fontSize = 18.sp)
        }
        Text(msg, color = P.Text2, fontFamily = Grotesk, fontSize = 14.sp, lineHeight = 20.sp)
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Chip("Back", Ic.Back, { act.closePlayer() }, bg = P.Line2)
            Chip("Try again", Ic.Loop, { ctrl.retry() }, active = true)
        }
    }
}

// ---------- the screen ----------

@Composable
fun PlayerScreen(act: MainActivity) {
    val ctrl = act.ctrl
    val landscape = LocalConfiguration.current.orientation == Configuration.ORIENTATION_LANDSCAPE
    var forcedFull by remember { mutableStateOf(false) }
    val full = landscape || forcedFull
    var controlsVisible by remember { mutableStateOf(true) }
    var poke by remember { mutableIntStateOf(0) }
    val interact: () -> Unit = { poke++; controlsVisible = true }

    LaunchedEffect(full) { act.setImmersive(full) }
    LaunchedEffect(controlsVisible, poke, ctrl.playing, ctrl.scrubbing, full) {
        if (full && controlsVisible && ctrl.playing && !ctrl.scrubbing) {
            delay(3000)
            controlsVisible = false
        }
    }
    BackHandler {
        if (forcedFull) {
            forcedFull = false
        } else {
            act.closePlayer()
        }
    }

    val onFullscreen: () -> Unit = {
        if (full) {
            if (landscape) act.setRotation(2)
            forcedFull = false
        } else {
            if (ctrl.videoAspect > 1.1f) {
                act.setRotation(1)
            } else {
                forcedFull = true
            }
        }
        Unit
    }

    Column(Modifier.fillMaxSize().background(Color.Black)) {
        if (!full) DockedTopBar(act)
        val videoMod = if (full) Modifier.fillMaxWidth().weight(1f)
        else Modifier.fillMaxWidth().aspectRatio(ctrl.videoAspect.coerceIn(0.75f, 2.4f))
        Box(videoMod.background(Color.Black)) {
            VideoSurface(ctrl)
            GestureLayer(ctrl, onTap = {
                if (full) {
                    controlsVisible = !controlsVisible; poke++
                } else {
                    ctrl.togglePlay()
                }
            })
            if (full) {
                val alpha by animateFloatAsState(if (controlsVisible) 1f else 0f, tween(220), label = "controls")
                if (alpha > 0.01f) {
                    Box(Modifier.matchParentSize().alpha(alpha)) {
                        OverlayControls(act, interact, onFullscreen)
                    }
                }
            }
            HoldHud(ctrl)
            NoticeToast(
                ctrl,
                Modifier.align(if (full) Alignment.TopCenter else Alignment.BottomCenter)
                    .padding(top = if (full) 76.dp else 0.dp, bottom = 12.dp, start = 12.dp, end = 12.dp),
            )
            FatalPanel(act)
        }
        if (!full) DockedControls(act, onFullscreen)
    }
}

@Composable
private fun DockedTopBar(act: MainActivity) {
    val ctrl = act.ctrl
    Row(
        Modifier.fillMaxWidth().background(P.Ink).statusBarsPadding().padding(start = 8.dp, end = 12.dp, top = 8.dp, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        IconBtn(Ic.Back, "Back", { act.closePlayer() }, bg = Color.Transparent, iconSize = 22.dp)
        Column(Modifier.weight(1f)) {
            Text(ctrl.name, color = P.Text, fontFamily = Grotesk, fontWeight = FontWeight.SemiBold, fontSize = 16.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(ctrl.info, color = P.Muted, fontFamily = Mono, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Chip(
            ROTATION_LABELS[act.rotationMode], Ic.Rotate, { act.cycleRotation() },
            desc = "Rotation: ${ROTATION_LABELS[act.rotationMode]}", bg = P.Surface,
        )
    }
}

@Composable
private fun OverlayControls(act: MainActivity, interact: () -> Unit, onFullscreen: () -> Unit) {
    val ctrl = act.ctrl
    Box(Modifier.fillMaxSize()) {
        // scrims
        Box(
            Modifier.fillMaxWidth().height(110.dp).align(Alignment.TopCenter)
                .background(Brush.verticalGradient(listOf(P.Ink.copy(alpha = 0.8f), Color.Transparent)))
        )
        Box(
            Modifier.fillMaxWidth().height(170.dp).align(Alignment.BottomCenter)
                .background(Brush.verticalGradient(listOf(Color.Transparent, P.Ink.copy(alpha = 0.92f))))
        )

        Column(Modifier.fillMaxSize().safeDrawingPadding().padding(horizontal = 20.dp, vertical = 12.dp)) {
            // top bar
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                IconBtn(Ic.Back, "Back", { act.closePlayer() }, bg = P.Ink.copy(alpha = 0.5f), iconSize = 22.dp)
                Column(Modifier.weight(1f)) {
                    Text(ctrl.name, color = P.Text, fontFamily = Grotesk, fontWeight = FontWeight.SemiBold, fontSize = 16.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(ctrl.info, color = P.Text2, fontFamily = Mono, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                Chip(
                    ROTATION_LABELS[act.rotationMode], Ic.Rotate, { interact(); act.cycleRotation() },
                    desc = "Rotation: ${ROTATION_LABELS[act.rotationMode]}",
                )
                Chip(
                    if (ctrl.markB != null || ctrl.markA != null) "Loop A–B" else "Loop", Ic.Loop,
                    { interact(); ctrl.toggleLoop() }, active = ctrl.loop,
                    desc = if (ctrl.loop) "Loop on" else "Loop off",
                )
            }

            // middle: hold hints + play/pause
            Box(Modifier.fillMaxWidth().weight(1f)) {
                Row(Modifier.align(Alignment.CenterStart), verticalAlignment = Alignment.CenterVertically) {
                    Icon(Ic.Rewind, null, tint = P.Text.copy(alpha = 0.7f), modifier = Modifier.size(30.dp))
                    Text("HOLD", color = P.Text.copy(alpha = 0.7f), fontFamily = Grotesk, fontSize = 11.sp, letterSpacing = 1.5.sp)
                }
                Row(Modifier.align(Alignment.CenterEnd), verticalAlignment = Alignment.CenterVertically) {
                    Text("HOLD", color = P.Text.copy(alpha = 0.7f), fontFamily = Grotesk, fontSize = 11.sp, letterSpacing = 1.5.sp)
                    Icon(Ic.Forward, null, tint = P.Text.copy(alpha = 0.7f), modifier = Modifier.size(30.dp))
                }
                IconBtn(
                    if (ctrl.playing) Ic.Pause else Ic.Play, if (ctrl.playing) "Pause" else "Play",
                    { interact(); ctrl.togglePlay() },
                    modifier = Modifier.align(Alignment.Center),
                    size = 80.dp, iconSize = 34.dp, bg = P.Ink.copy(alpha = 0.55f), borderColor = P.Text.copy(alpha = 0.35f),
                )
            }

            // bottom
            TimesRow(ctrl, P.Text2)
            Timeline(ctrl, onInteract = interact)
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                IconBtn(if (ctrl.muted) Ic.Mute else Ic.Volume, if (ctrl.muted) "Unmute" else "Mute", { interact(); ctrl.toggleMute() },
                    tint = if (ctrl.muted) P.Coral else P.Text)
                GradSlider(
                    ctrl.volume, { ctrl.setVolume(it) }, "Volume ${(ctrl.volume * 100).toInt()} percent",
                    Modifier.weight(0.6f), dimmed = ctrl.muted, onInteract = interact,
                )
                Box(Modifier.width(1.dp).height(24.dp).background(P.Text.copy(alpha = 0.2f)))
                Icon(Ic.Speed, null, tint = P.Text, modifier = Modifier.size(20.dp))
                GradSlider(
                    speedToPos(ctrl.speed), { ctrl.setSpeed(posToSpeed(it)) }, "Speed ${"%.2f".format(ctrl.speed)}x",
                    Modifier.weight(1f), speedStyle = true, onInteract = interact,
                )
                SpeedLabel(ctrl, interact)
                MarkerButton("[ A", P.Red, "Set loop start A", { interact(); ctrl.setA() })
                MarkerButton("B ]", P.Teal, "Set loop end B", { interact(); ctrl.setB() })
                if (ctrl.markA != null || ctrl.markB != null) {
                    IconBtn(Ic.Close, "Clear loop markers", { interact(); ctrl.clearMarks() }, shape = RoundedCornerShape(12.dp), iconSize = 18.dp)
                }
                IconBtn(Ic.ExitFullscreen, "Exit fullscreen", { interact(); onFullscreen() }, shape = RoundedCornerShape(12.dp))
            }
        }
    }
}

@Composable
private fun SpeedLabel(ctrl: PlayerController, interact: () -> Unit) {
    Box(
        Modifier.height(44.dp).widthIn(min = 52.dp).clip(RoundedCornerShape(8.dp))
            .clickable(role = Role.Button, onClickLabel = "Reset speed") { interact(); ctrl.setSpeed(1f) },
        contentAlignment = Alignment.Center,
    ) {
        Text("%.2f×".format(ctrl.speed), color = P.Yellow, fontFamily = Mono, fontSize = 14.sp)
    }
}

/** Press & hold = ramping seek; a short tap steps one frame. */
@Composable
private fun HoldButton(ctrl: PlayerController, dir: Int) {
    val scope = rememberCoroutineScope()
    Box(
        Modifier.size(64.dp).clip(CircleShape).background(P.Surface).border(1.dp, P.Line2, CircleShape)
            .semantics {
                role = Role.Button
                contentDescription = if (dir > 0) "Hold to fast forward, tap for next frame" else "Hold to rewind, tap for previous frame"
            }
            .pointerInput(dir) {
                detectTapGestures(onPress = {
                    var holding = false
                    val job: Job = scope.launch {
                        delay(HOLD_DELAY_MS)
                        holding = true
                        ctrl.startHold(dir)
                    }
                    val released = tryAwaitRelease()
                    job.cancel()
                    if (holding) ctrl.endHold() else if (released) ctrl.stepFrame(dir)
                })
            },
        contentAlignment = Alignment.Center,
    ) {
        Icon(if (dir > 0) Ic.Forward else Ic.Rewind, null, tint = P.Text, modifier = Modifier.size(28.dp))
    }
}

@Composable
private fun ColumnScope.DockedControls(act: MainActivity, onFullscreen: () -> Unit) {
    val ctrl = act.ctrl
    Column(
        Modifier.fillMaxWidth().weight(1f).background(P.Ink).verticalScroll(rememberScrollState()).navigationBarsPadding().padding(horizontal = 20.dp, vertical = 16.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Timeline(ctrl)
        TimesRow(ctrl, P.Text2)
        Row(
            Modifier.fillMaxWidth().padding(top = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(28.dp, Alignment.CenterHorizontally),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            HoldButton(ctrl, -1)
            Box(
                Modifier.size(84.dp).clip(CircleShape)
                    .background(Brush.linearGradient(listOf(P.Red, P.Coral, P.Yellow)))
                    .clickable(role = Role.Button) { ctrl.togglePlay() }
                    .semantics { contentDescription = if (ctrl.playing) "Pause" else "Play" },
                contentAlignment = Alignment.Center,
            ) { Icon(if (ctrl.playing) Ic.Pause else Ic.Play, null, tint = P.Ink, modifier = Modifier.size(36.dp)) }
            HoldButton(ctrl, 1)
        }
        Spacer(Modifier.height(16.dp))
        Column(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(18.dp)).background(P.Surface)
                .border(1.dp, P.Line, RoundedCornerShape(18.dp)).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Speed", color = P.Muted, fontFamily = Grotesk, fontSize = 13.sp, modifier = Modifier.width(64.dp))
                GradSlider(
                    speedToPos(ctrl.speed), { ctrl.setSpeed(posToSpeed(it)) }, "Speed ${"%.2f".format(ctrl.speed)}x",
                    Modifier.weight(1f), speedStyle = true,
                )
                SpeedLabel(ctrl) {}
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Volume", color = P.Muted, fontFamily = Grotesk, fontSize = 13.sp, modifier = Modifier.width(64.dp))
                GradSlider(
                    ctrl.volume, { ctrl.setVolume(it) }, "Volume ${(ctrl.volume * 100).toInt()} percent",
                    Modifier.weight(1f), dimmed = ctrl.muted,
                )
                IconBtn(
                    if (ctrl.muted) Ic.Mute else Ic.Volume, if (ctrl.muted) "Unmute" else "Mute", { ctrl.toggleMute() },
                    size = 48.dp, shape = RoundedCornerShape(12.dp),
                    bg = if (ctrl.muted) P.Red.copy(alpha = 0.2f) else Color.White.copy(alpha = 0.06f),
                    tint = if (ctrl.muted) P.Coral else P.Text,
                )
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                MarkerButton("[ A", P.Red, "Set loop start A", { ctrl.setA() }, Modifier.weight(1f))
                MarkerButton("B ]", P.Teal, "Set loop end B", { ctrl.setB() }, Modifier.weight(1f))
                if (ctrl.markA != null || ctrl.markB != null) {
                    IconBtn(Ic.Close, "Clear loop markers", { ctrl.clearMarks() }, modifier = Modifier.weight(1f),
                        shape = RoundedCornerShape(12.dp), bg = Color.Transparent, borderColor = P.Line2, iconSize = 18.dp)
                }
                IconBtn(
                    Ic.Loop, if (ctrl.loop) "Loop on" else "Loop off", { ctrl.toggleLoop() }, modifier = Modifier.weight(1f),
                    shape = RoundedCornerShape(12.dp),
                    bg = if (ctrl.loop) P.Yellow else Color.Transparent, tint = if (ctrl.loop) P.Ink else P.Text,
                    borderColor = if (ctrl.loop) null else P.Line2,
                )
                IconBtn(
                    Ic.Fullscreen, "Fullscreen", onFullscreen, modifier = Modifier.weight(1f),
                    shape = RoundedCornerShape(12.dp), bg = Color.Transparent, borderColor = P.Line2,
                )
            }
        }
    }
}
