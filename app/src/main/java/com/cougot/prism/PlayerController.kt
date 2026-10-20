package com.cougot.prism

import android.content.Context
import android.net.Uri
import android.os.SystemClock
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.common.Tracks
import androidx.media3.common.VideoSize
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.SeekParameters
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.exoplayer.upstream.DefaultLoadErrorHandlingPolicy
import androidx.media3.extractor.DefaultExtractorsFactory
import androidx.media3.extractor.ts.DefaultTsPayloadReaderFactory
import com.cougot.prism.ui.fmtTime
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/**
 * Wraps ExoPlayer and exposes Compose state. Adds:
 *  - damage recovery: on a recoverable error or a stall, skip ahead (1s, 2s, 4s… up to 32s) and
 *    re-prepare, recording the skipped range; give up with a message after MAX_RECOVERIES in a row.
 *  - A/B loop markers, hold-to-seek with ramping, scrubbing.
 */
class PlayerController(context: Context, private val scope: CoroutineScope) {

    companion object {
        val RATES = intArrayOf(2, 4, 8, 16, 32, 64)
        const val RAMP_MS = 900L
        const val STEP_MS = 120L
        const val STALL_MS = 6000L
        const val MAX_RECOVERIES = 6
    }

    val player: ExoPlayer

    var uri by mutableStateOf<Uri?>(null); private set
    var name by mutableStateOf(""); private set
    var playing by mutableStateOf(false); private set
    var position by mutableLongStateOf(0L); private set
    var duration by mutableLongStateOf(0L); private set
    var buffered by mutableLongStateOf(0L); private set
    var speed by mutableFloatStateOf(1f); private set
    var volume by mutableFloatStateOf(1f); private set
    var muted by mutableStateOf(false); private set
    var loop by mutableStateOf(false); private set
    var markA by mutableStateOf<Long?>(null); private set
    var markB by mutableStateOf<Long?>(null); private set
    var info by mutableStateOf(""); private set
    var videoAspect by mutableFloatStateOf(16f / 9f); private set
    var notice by mutableStateOf<String?>(null); private set
    var noticeId by mutableIntStateOf(0); private set
    var noticeWarn by mutableStateOf(false); private set
    var fatal by mutableStateOf<String?>(null); private set
    val damaged = mutableStateListOf<LongRange>()
    var holdDir by mutableIntStateOf(0); private set
    var holdLevel by mutableIntStateOf(0); private set
    var holdDelta by mutableLongStateOf(0L); private set
    var scrubbing by mutableStateOf(false); private set

    private var recoveries = 0
    private var recoveredAt = 0L
    private var lastGoodPos = 0L
    private var lastTickPos = -1L
    private var stallSince = 0L
    private var resumeAfter = false
    private var lastScrubSeek = 0L
    private var holdJob: Job? = null
    private var holdTarget = 0L
    private var holdFastPlay = false
    private var warnedTracks = false

    init {
        val extractors = DefaultExtractorsFactory()
            .setConstantBitrateSeekingEnabled(true)
            .setConstantBitrateSeekingAlwaysEnabled(true)
            .setTsExtractorFlags(
                DefaultTsPayloadReaderFactory.FLAG_ALLOW_NON_IDR_KEYFRAMES or
                    DefaultTsPayloadReaderFactory.FLAG_DETECT_ACCESS_UNITS
            )
        val renderers = DefaultRenderersFactory(context)
            .setEnableDecoderFallback(true)
        val sources = DefaultMediaSourceFactory(context, extractors)
            .setLoadErrorHandlingPolicy(DefaultLoadErrorHandlingPolicy(2))
        player = ExoPlayer.Builder(context, renderers)
            .setMediaSourceFactory(sources)
            .setAudioAttributes(AudioAttributes.DEFAULT, true)
            .setHandleAudioBecomingNoisy(true)
            .build()
        player.addListener(object : Player.Listener {
            override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
                playing = playWhenReady
            }

            override fun onPlaybackStateChanged(playbackState: Int) {
                if (playbackState == Player.STATE_READY) {
                    val d = player.duration
                    if (d != C.TIME_UNSET && d > 0) duration = d
                    updateInfo()
                }
                if (playbackState == Player.STATE_ENDED && loop && holdDir == 0) {
                    player.seekTo(markA ?: 0L)
                    player.play()
                }
            }

            override fun onPlayerError(error: PlaybackException) = handleError(error)

            override fun onVideoSizeChanged(videoSize: VideoSize) {
                if (videoSize.width > 0 && videoSize.height > 0) {
                    videoAspect = videoSize.width * videoSize.pixelWidthHeightRatio / videoSize.height
                    updateInfo()
                }
            }

            override fun onTracksChanged(tracks: Tracks) {
                updateInfo()
                if (warnedTracks) return
                val video = tracks.groups.filter { it.type == C.TRACK_TYPE_VIDEO }
                val audio = tracks.groups.filter { it.type == C.TRACK_TYPE_AUDIO }
                if (video.isNotEmpty() && video.none { it.isSupported }) {
                    warnedTracks = true
                    val codec = codecName(video[0].getTrackFormat(0).sampleMimeType)
                    showNotice("This phone can't decode $codec video — playing audio only", warn = true)
                } else if (audio.isNotEmpty() && audio.none { it.isSupported }) {
                    warnedTracks = true
                    val codec = codecName(audio[0].getTrackFormat(0).sampleMimeType)
                    showNotice("$codec audio isn't supported — playing video silently", warn = true)
                }
            }
        })
        scope.launch {
            while (isActive) {
                tick()
                delay(33)
            }
        }
    }

    // ---- file lifecycle ----

    fun open(u: Uri, displayName: String, startAt: Long) {
        holdJob?.cancel(); holdDir = 0; scrubbing = false
        uri = u; name = displayName
        fatal = null; notice = null; info = ""
        markA = null; markB = null; damaged.clear()
        duration = 0; buffered = 0; position = startAt
        recoveries = 0; lastGoodPos = startAt; stallSince = 0; lastTickPos = -1
        warnedTracks = false
        updateRepeat()
        player.setSeekParameters(SeekParameters.EXACT)
        player.setPlaybackParameters(PlaybackParameters(speed))
        applyVolume()
        player.setMediaItem(MediaItem.fromUri(u), startAt)
        player.prepare()
        player.play()
    }

    fun retry() {
        val u = uri ?: return
        val at = position
        val n = name
        open(u, n, at)
    }

    fun close() {
        holdJob?.cancel(); holdDir = 0; scrubbing = false
        player.stop()
        player.clearMediaItems()
        uri = null
    }

    fun release() = player.release()

    // ---- transport ----

    private fun ensurePrepared() {
        if (player.playbackState == Player.STATE_IDLE && uri != null && fatal == null) player.prepare()
    }

    private fun clampPos(ms: Long): Long =
        if (duration > 0) ms.coerceIn(0L, duration) else ms.coerceAtLeast(0L)

    fun togglePlay() {
        if (player.playWhenReady) {
            player.pause(); return
        }
        ensurePrepared()
        val b = markB
        if (player.playbackState == Player.STATE_ENDED) {
            player.seekTo(markA ?: 0L)
        } else if (b != null && !loop && position >= b - 100) {
            player.seekTo(markA ?: 0L)
        }
        player.play()
    }

    fun pause() = player.pause()

    fun seekTo(ms: Long) {
        ensurePrepared()
        val t = clampPos(ms)
        position = t
        player.seekTo(t)
    }

    fun stepFrame(dir: Int) {
        if (uri == null) return
        player.pause()
        val fps = player.videoFormat?.frameRate?.takeIf { it > 0f } ?: 30f
        val step = (1000f / fps).toLong().coerceAtLeast(1L)
        seekTo(position + dir * step)
    }

    fun changeSpeed(s: Float) {
        speed = s.coerceIn(0.1f, 4f)
        if (holdDir == 0) player.setPlaybackParameters(PlaybackParameters(speed))
    }

    fun changeVolume(v: Float) {
        volume = v.coerceIn(0f, 1f)
        if (volume > 0f) muted = false
        applyVolume()
    }

    fun toggleMute() {
        muted = !muted
        applyVolume()
    }

    private fun applyVolume() {
        player.volume = if (muted) 0f else volume
    }

    fun toggleLoop() {
        loop = !loop
        updateRepeat()
    }

    private fun updateRepeat() {
        player.repeatMode =
            if (loop && markA == null && markB == null) Player.REPEAT_MODE_ONE else Player.REPEAT_MODE_OFF
    }

    // ---- A/B markers ----

    fun setA() {
        val p = position
        markA = p
        val b = markB
        if (b != null && b <= p + 100) markB = null
        updateRepeat()
        showNotice("Loop start A at ${fmtTime(p)}")
    }

    fun setB() {
        val p = position
        val a = markA ?: 0L
        if (p <= a + 100) {
            showNotice("B must be after A")
            return
        }
        markB = p
        loop = true
        updateRepeat()
        seekTo(a)
        showNotice("Looping ${fmtTime(a)} – ${fmtTime(p)}")
    }

    fun clearMarks() {
        markA = null; markB = null
        updateRepeat()
    }

    // ---- scrubbing ----

    fun beginScrub() {
        if (scrubbing || uri == null) return
        scrubbing = true
        resumeAfter = player.playWhenReady
        player.pause()
        ensurePrepared()
        player.setSeekParameters(
            if (duration in 1..120_000) SeekParameters.EXACT else SeekParameters.CLOSEST_SYNC
        )
    }

    fun scrubTo(ms: Long) {
        if (!scrubbing) return
        val t = clampPos(ms)
        position = t
        val now = SystemClock.uptimeMillis()
        if (now - lastScrubSeek > 50) {
            lastScrubSeek = now
            player.seekTo(t)
        }
    }

    fun endScrub() {
        if (!scrubbing) return
        player.setSeekParameters(SeekParameters.EXACT)
        player.seekTo(position)
        scrubbing = false
        lastTickPos = -1; stallSince = 0
        if (resumeAfter) player.play()
    }

    // ---- hold-to-seek with ramping ----

    fun startHold(dir: Int) {
        if (uri == null || fatal != null || holdDir != 0) return
        holdJob?.cancel()
        resumeAfter = player.playWhenReady
        ensurePrepared()
        holdDir = dir; holdLevel = 0; holdDelta = 0
        val start = player.currentPosition
        holdTarget = start
        holdFastPlay = false
        holdJob = scope.launch {
            val t0 = SystemClock.uptimeMillis()
            while (isActive) {
                val level = ((SystemClock.uptimeMillis() - t0) / RAMP_MS).toInt().coerceAtMost(RATES.lastIndex)
                val rate = RATES[level]
                holdLevel = level
                if (dir > 0 && rate <= 4) {
                    // Low forward rates: real playback at 2x/4x looks smoother than stepping.
                    if (!holdFastPlay || player.playbackParameters.speed != rate.toFloat()) {
                        holdFastPlay = true
                        player.volume = 0f
                        player.setPlaybackParameters(PlaybackParameters(rate.toFloat()))
                        player.play()
                    }
                    holdTarget = player.currentPosition
                } else {
                    if (holdFastPlay) {
                        holdFastPlay = false
                        player.pause()
                        holdTarget = player.currentPosition
                        player.setPlaybackParameters(PlaybackParameters(speed))
                    } else if (player.playWhenReady) {
                        player.pause()
                    }
                    player.setSeekParameters(SeekParameters.CLOSEST_SYNC)
                    holdTarget = clampPos(holdTarget + dir * rate * STEP_MS)
                    player.seekTo(holdTarget)
                }
                position = holdTarget
                holdDelta = holdTarget - start
                delay(STEP_MS)
            }
        }
    }

    fun endHold() {
        if (holdDir == 0) return
        holdJob?.cancel(); holdJob = null
        holdDir = 0
        val t = if (holdFastPlay) player.currentPosition else holdTarget
        holdFastPlay = false
        player.setPlaybackParameters(PlaybackParameters(speed))
        applyVolume()
        player.setSeekParameters(SeekParameters.EXACT)
        player.seekTo(t)
        position = t
        lastTickPos = -1; stallSince = 0
        if (resumeAfter) player.play() else player.pause()
    }

    // ---- damage handling ----

    private fun handleError(e: PlaybackException) {
        val hard = when (e.errorCode) {
            PlaybackException.ERROR_CODE_IO_FILE_NOT_FOUND,
            PlaybackException.ERROR_CODE_IO_NO_PERMISSION,
            PlaybackException.ERROR_CODE_PARSING_CONTAINER_UNSUPPORTED -> true
            else -> false
        }
        if (hard) {
            fatal = describe(e)
            return
        }
        skipAhead(describe(e))
    }

    private fun skipAhead(reason: String) {
        val from = maxOf(player.currentPosition, lastGoodPos)
        recoveries++
        recoveredAt = from
        if (recoveries > MAX_RECOVERIES) {
            player.stop()
            fatal = "Couldn't get past damaged data near ${fmtTime(from, false)}.\n$reason"
            return
        }
        val jump = 1000L shl (recoveries - 1).coerceAtMost(5)
        val to = from + jump
        val dur = duration
        if (dur > 0 && to >= dur) {
            damaged.add(from..dur)
            player.stop()
            position = dur
            showNotice("The rest of the file is unreadable (from ${fmtTime(from, false)})", warn = true)
            if (loop) {
                recoveries = 0
                seekTo(markA ?: 0L)
                player.play()
            }
            return
        }
        damaged.add(from..to)
        showNotice("Skipped damaged data ${fmtTime(from, false)}–${fmtTime(to, false)}", warn = true)
        lastGoodPos = to
        stallSince = 0
        player.seekTo(to)
        player.prepare()
        player.playWhenReady = true
    }

    private fun tick() {
        if (uri == null) return
        val st = player.playbackState
        val pos = player.currentPosition
        val d = player.duration
        if (d != C.TIME_UNSET && d > 0) duration = d
        buffered = player.bufferedPosition
        if (holdDir == 0 && !scrubbing) position = pos

        if (st == Player.STATE_READY && player.isPlaying) {
            lastGoodPos = pos
            if (recoveries > 0 && pos > recoveredAt + 3000) recoveries = 0
        }

        // A/B enforcement
        val b = markB
        if (b != null && holdDir == 0 && !scrubbing && player.isPlaying && pos >= b) {
            if (loop) player.seekTo(markA ?: 0L) else {
                player.pause(); player.seekTo(b)
            }
        }

        // Watchdog: never sit forever on a broken spot.
        val now = SystemClock.uptimeMillis()
        val trying = player.playWhenReady && holdDir == 0 && !scrubbing && fatal == null
        val progressing = pos != lastTickPos
        lastTickPos = pos
        val stuck = (st == Player.STATE_BUFFERING) || (st == Player.STATE_READY && player.isPlaying && !progressing)
        if (trying && stuck) {
            if (stallSince == 0L) stallSince = now
            else if (now - stallSince > STALL_MS) {
                stallSince = 0
                skipAhead("Playback stalled")
            }
        } else if (progressing || !trying) {
            stallSince = 0
        }
    }

    // ---- text ----

    fun showNotice(msg: String, warn: Boolean = false) {
        notice = msg
        noticeWarn = warn
        noticeId++
    }

    fun dismissNotice() {
        notice = null
    }

    private fun updateInfo() {
        val parts = mutableListOf<String>()
        val v = player.videoFormat
        if (v != null) {
            parts += codecName(v.sampleMimeType)
            if (v.width > 0 && v.height > 0) parts += "${v.width}×${v.height}"
            if (v.frameRate > 0) parts += "%.2f fps".format(v.frameRate).replace(".00", "")
        } else {
            val a = player.audioFormat
            if (a != null) parts += "Audio · " + codecName(a.sampleMimeType)
        }
        info = parts.joinToString(" · ")
    }

    private fun codecName(mime: String?): String = when (mime) {
        null -> "Unknown"
        "video/avc" -> "H.264"
        "video/hevc" -> "HEVC"
        "video/x-vnd.on2.vp9" -> "VP9"
        "video/x-vnd.on2.vp8" -> "VP8"
        "video/av01" -> "AV1"
        "video/mp4v-es" -> "MPEG-4"
        "video/3gpp" -> "H.263"
        "video/mpeg2" -> "MPEG-2"
        "video/dolby-vision" -> "Dolby Vision"
        else -> mime.substringAfter('/').removePrefix("x-").uppercase()
    }

    private fun describe(e: PlaybackException): String = when (e.errorCode) {
        PlaybackException.ERROR_CODE_IO_FILE_NOT_FOUND -> "File not found — it may have been moved or deleted."
        PlaybackException.ERROR_CODE_IO_NO_PERMISSION -> "No permission to read this file. Open it again from the picker."
        PlaybackException.ERROR_CODE_PARSING_CONTAINER_UNSUPPORTED -> "This file format isn't supported."
        PlaybackException.ERROR_CODE_PARSING_CONTAINER_MALFORMED -> "Damaged container data."
        PlaybackException.ERROR_CODE_DECODING_FAILED -> "The decoder hit damaged frames."
        PlaybackException.ERROR_CODE_DECODER_INIT_FAILED -> "No working decoder for this codec."
        PlaybackException.ERROR_CODE_DECODING_FORMAT_UNSUPPORTED,
        PlaybackException.ERROR_CODE_DECODING_FORMAT_EXCEEDS_CAPABILITIES -> "This phone's decoder can't handle this format."
        else -> e.errorCodeName
    }
}
