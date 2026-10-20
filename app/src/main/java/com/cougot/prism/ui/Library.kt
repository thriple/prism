package com.cougot.prism.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.cougot.prism.MainActivity
import com.cougot.prism.RecentItem

@Composable
fun App(act: MainActivity) {
    MaterialTheme(
        colorScheme = darkColorScheme(primary = P.Yellow, background = P.Ink, surface = P.Surface)
    ) {
        if (act.ctrl.uri == null) LibraryScreen(act) else PlayerScreen(act)
    }
}

@Composable
fun Logo(modifier: Modifier = Modifier) {
    Canvas(modifier) {
        val s = size.minDimension
        val brush = Brush.linearGradient(listOf(P.Red, P.Yellow, P.Teal), Offset(0f, 0f), Offset(s, 0f))
        val sw = s * 0.083f
        drawCircle(brush, radius = s * 0.44f, center = Offset(s * 0.5f, s * 0.5f), style = Stroke(sw))
        drawCircle(brush, radius = s * 0.28f, center = Offset(s * 0.56f, s * 0.5f), style = Stroke(sw), alpha = 0.85f)
        drawCircle(P.Yellow, radius = s * 0.11f, center = Offset(s * 0.61f, s * 0.5f))
    }
}

@Composable
fun LibraryScreen(act: MainActivity) {
    val recents = act.recents.items
    Box(Modifier.fillMaxSize().background(P.Ink)) {
        Canvas(Modifier.offset((-160).dp, (-180).dp).size(520.dp)) {
            drawCircle(
                Brush.radialGradient(
                    0f to P.Yellow.copy(alpha = 0.22f),
                    0.55f to P.Red.copy(alpha = 0.16f),
                    0.9f to P.DeepTeal.copy(alpha = 0.10f),
                    1f to Color.Transparent,
                    center = Offset(size.width * 0.6f, size.height * 0.55f),
                    radius = size.width / 2,
                )
            )
        }
        LazyColumn(
            Modifier.fillMaxSize().systemBarsPadding(),
            contentPadding = PaddingValues(start = 24.dp, end = 24.dp, top = 24.dp, bottom = 32.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            item {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Logo(Modifier.size(36.dp))
                    Spacer(Modifier.width(12.dp))
                    Text("Prism", color = P.Text, fontFamily = Grotesk, fontWeight = FontWeight.Bold, fontSize = 28.sp)
                }
            }
            item {
                Column(Modifier.padding(top = 20.dp)) {
                    Row(
                        Modifier.fillMaxWidth().height(64.dp).clip(RoundedCornerShape(20.dp))
                            .background(P.prismBrush)
                            .clickable(role = Role.Button) { act.pickFile() },
                        horizontalArrangement = Arrangement.Center,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Icon(Ic.Folder, null, tint = P.Ink, modifier = Modifier.size(22.dp))
                        Spacer(Modifier.width(10.dp))
                        Text("Open video", color = P.Ink, fontFamily = Grotesk, fontWeight = FontWeight.Bold, fontSize = 18.sp)
                    }
                    Text(
                        "MP4 · MKV · WebM · MOV · TS · AVI · FLV · 3GP — or “Open with” from any app",
                        color = P.Muted, fontFamily = Grotesk, fontSize = 13.sp, textAlign = TextAlign.Center,
                        modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
                    )
                }
            }
            item {
                Row(Modifier.fillMaxWidth().padding(top = 20.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        "RECENT", color = P.Muted, fontFamily = Grotesk, fontWeight = FontWeight.SemiBold,
                        fontSize = 15.sp, letterSpacing = 1.5.sp, modifier = Modifier.weight(1f),
                    )
                    if (recents.isNotEmpty()) {
                        Box(
                            Modifier.heightIn(min = 44.dp).clickable(role = Role.Button) { act.recents.clear() }
                                .padding(horizontal = 8.dp),
                            contentAlignment = Alignment.Center,
                        ) { Text("Clear", color = P.Yellow, fontFamily = Grotesk, fontSize = 14.sp) }
                    }
                }
            }
            if (recents.isEmpty()) {
                item {
                    Text(
                        "Nothing yet. Open a video here, or pick Prism from “Open with” in your file manager.",
                        color = P.Muted, fontFamily = Grotesk, fontSize = 14.sp,
                    )
                }
            }
            items(recents, key = { it.uri }) { r ->
                RecentRow(r, onOpen = { act.openRecent(r) }, onRemove = { act.recents.remove(r.uri) })
            }
        }
    }
}

private fun extOf(name: String): String =
    name.substringAfterLast('.', "").uppercase().take(5).ifBlank { "VIDEO" }

private fun thumbBrush(ext: String, damaged: Boolean): Brush {
    val c = when {
        damaged -> listOf(P.Maroon, P.Red)
        ext == "MP4" || ext == "M4V" -> listOf(P.DeepTeal, P.Teal)
        ext == "MKV" -> listOf(P.Red, P.Coral)
        ext == "WEBM" -> listOf(P.Coral, P.Yellow)
        ext == "TS" || ext == "M2TS" || ext == "MTS" -> listOf(P.Maroon, P.Coral)
        ext == "MOV" -> listOf(P.Yellow, P.Mint)
        else -> listOf(P.DeepTeal, P.Coral)
    }
    return Brush.linearGradient(c)
}

@Composable
private fun RecentRow(r: RecentItem, onOpen: () -> Unit, onRemove: () -> Unit) {
    val ext = extOf(r.name)
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(P.Surface)
            .border(1.dp, P.Line, RoundedCornerShape(16.dp))
            .clickable(role = Role.Button, onClick = onOpen).padding(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.width(88.dp).height(56.dp).clip(RoundedCornerShape(10.dp)).background(thumbBrush(ext, r.damaged))) {
            if (r.durationMs > 0) {
                Text(
                    fmtTime(r.durationMs, false), color = P.Text, fontFamily = Mono, fontSize = 11.sp,
                    modifier = Modifier.align(Alignment.BottomEnd).padding(4.dp)
                        .background(P.Ink.copy(alpha = 0.8f), RoundedCornerShape(4.dp)).padding(horizontal = 5.dp),
                )
            }
        }
        Spacer(Modifier.width(14.dp))
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(
                r.name, color = P.Text, fontFamily = Grotesk, fontWeight = FontWeight.Medium, fontSize = 15.sp,
                maxLines = 1, overflow = TextOverflow.Ellipsis,
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    ext, color = P.Muted, fontFamily = Mono, fontSize = 12.sp,
                    modifier = Modifier.border(1.dp, Color(0xFF2E3438), RoundedCornerShape(4.dp)).padding(horizontal = 5.dp),
                )
                Spacer(Modifier.width(8.dp))
                Text(
                    if (r.damaged) "Damaged · partial" else r.info, color = if (r.damaged) P.Coral else P.Muted,
                    fontFamily = Grotesk, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis,
                )
            }
            val frac = if (r.durationMs > 0) (r.positionMs.toFloat() / r.durationMs).coerceIn(0f, 1f) else 0f
            Box(Modifier.fillMaxWidth().height(3.dp).clip(RoundedCornerShape(2.dp)).background(P.Line2)) {
                Box(Modifier.fillMaxWidth(frac).height(3.dp).background(P.warmBrush))
            }
        }
        IconBtn(Ic.Close, "Remove from recent", onRemove, bg = Color.Transparent, tint = P.Muted, iconSize = 16.dp)
    }
}
