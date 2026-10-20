package com.cougot.prism.ui

import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import com.cougot.prism.R

object P {
    val Ink = Color(0xFF07090A)
    val Surface = Color(0xFF121416)
    val Line = Color(0xFF1C2023)
    val Line2 = Color(0xFF23272A)
    val Text = Color(0xFFF3EEE4)
    val Text2 = Color(0xFFC9C4BA)
    val Muted = Color(0xFF9AA3A6)
    val Red = Color(0xFFE0353A)
    val Coral = Color(0xFFF2785C)
    val Yellow = Color(0xFFF4DE8C)
    val Mint = Color(0xFF8FD3C8)
    val Teal = Color(0xFF3FB3AE)
    val DeepTeal = Color(0xFF11504F)
    val Maroon = Color(0xFF3B1416)

    val prism = listOf(Red, Coral, Yellow, Mint, Teal)
    val prismBrush = Brush.horizontalGradient(prism)
    val warmBrush = Brush.horizontalGradient(listOf(Red, Yellow))
    val speedBrush = Brush.horizontalGradient(listOf(Teal, Yellow, Red))
}

val Grotesk = FontFamily(
    Font(R.font.space_grotesk_regular, FontWeight.Normal),
    Font(R.font.space_grotesk_medium, FontWeight.Medium),
    Font(R.font.space_grotesk_medium, FontWeight.SemiBold),
    Font(R.font.space_grotesk_bold, FontWeight.Bold),
)
val Mono = FontFamily(Font(R.font.jetbrains_mono_regular, FontWeight.Normal))

fun fmtTime(ms: Long, tenths: Boolean = true): String {
    val t = ms.coerceAtLeast(0)
    val totalSec = t / 1000
    val h = totalSec / 3600
    val m = (totalSec % 3600) / 60
    val s = totalSec % 60
    val d = (t % 1000) / 100
    val base = if (h > 0) "%d:%02d:%02d".format(h, m, s) else "%02d:%02d".format(m, s)
    return if (tenths) "$base.$d" else base
}
