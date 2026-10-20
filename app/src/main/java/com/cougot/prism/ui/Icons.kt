package com.cougot.prism.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.unit.dp

object Ic {
    private fun build(filled: Boolean, width: Float, paths: Array<out String>): ImageVector {
        val b = ImageVector.Builder(
            defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f,
        )
        for (d in paths) {
            b.addPath(
                pathData = addPathNodes(d),
                fill = if (filled) SolidColor(Color.White) else null,
                stroke = if (filled) null else SolidColor(Color.White),
                strokeLineWidth = width,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            )
        }
        return b.build()
    }

    private fun stroke(vararg d: String) = build(false, 2f, d)
    private fun filled(vararg d: String) = build(true, 0f, d)

    val Back = stroke("M15 18l-6-6 6-6")
    val Folder = stroke("M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z", "M12 10.5v5M9.5 13h5")
    val Rotate = stroke(
        "M9 3h6a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H9a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z",
        "M3 12a9 9 0 0 1 3-6.7", "M21 12a9 9 0 0 1-3 6.7",
    )
    val Loop = stroke("M17 2l4 4-4 4", "M3 11V9a3 3 0 0 1 3-3h15", "M7 22l-4-4 4-4", "M21 13v2a3 3 0 0 1-3 3H3")
    val Play = filled("M7 4.5v15a1 1 0 0 0 1.5 0.9l12-7.5a1 1 0 0 0 0-1.8l-12-7.5A1 1 0 0 0 7 4.5z")
    val Pause = filled(
        "M7 4h2a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1z",
        "M15 4h2a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1h-2a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1z",
    )
    val Volume = stroke("M11 5L6 9H2v6h4l5 4z", "M15.5 8.5a5 5 0 0 1 0 7", "M19 5a10 10 0 0 1 0 14")
    val Mute = stroke("M11 5L6 9H2v6h4l5 4z", "M22 9l-6 6", "M16 9l6 6")
    val Speed = stroke("M12 14l4-4", "M3.3 19a10 10 0 1 1 17.4 0")
    val Close = stroke("M6 6l12 12", "M18 6L6 18")
    val Fullscreen = stroke("M4 9V4h5", "M20 9V4h-5", "M4 15v5h5", "M20 15v5h-5")
    val ExitFullscreen = stroke("M9 4v5H4", "M15 4v5h5", "M9 20v-5H4", "M15 20v-5h5")
    val Warning = stroke("M12 9v4", "M12 17h0.01", "M10.3 3.9L1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z")
    val Rewind = stroke("M11 17l-5-5 5-5", "M18 17l-5-5 5-5")
    val Forward = stroke("M13 17l5-5-5-5", "M6 17l5-5-5-5")
}
