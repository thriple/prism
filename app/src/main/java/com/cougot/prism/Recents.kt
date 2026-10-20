package com.cougot.prism

import android.content.Context
import androidx.compose.runtime.mutableStateListOf
import org.json.JSONArray
import org.json.JSONObject

data class RecentItem(
    val uri: String,
    val name: String,
    val durationMs: Long,
    val positionMs: Long,
    val info: String,
    val damaged: Boolean,
)

class Recents(context: Context) {
    private val prefs = context.getSharedPreferences("prism", Context.MODE_PRIVATE)
    val items = mutableStateListOf<RecentItem>()

    init {
        try {
            val arr = JSONArray(prefs.getString("recents", "[]"))
            for (i in 0 until arr.length()) {
                val o = arr.getJSONObject(i)
                items.add(
                    RecentItem(
                        o.getString("uri"), o.optString("name"), o.optLong("dur"),
                        o.optLong("pos"), o.optString("info"), o.optBoolean("dmg"),
                    )
                )
            }
        } catch (_: Exception) {
        }
    }

    fun find(uri: String): RecentItem? = items.firstOrNull { it.uri == uri }

    fun upsert(item: RecentItem) {
        items.removeAll { it.uri == item.uri }
        items.add(0, item)
        while (items.size > 30) items.removeAt(items.lastIndex)
        save()
    }

    fun remove(uri: String) {
        items.removeAll { it.uri == uri }
        save()
    }

    fun clear() {
        items.clear()
        save()
    }

    private fun save() {
        val arr = JSONArray()
        items.forEach {
            arr.put(
                JSONObject()
                    .put("uri", it.uri).put("name", it.name).put("dur", it.durationMs)
                    .put("pos", it.positionMs).put("info", it.info).put("dmg", it.damaged)
            )
        }
        prefs.edit().putString("recents", arr.toString()).apply()
    }

    var rotation: Int
        get() = prefs.getInt("rotation", 0)
        set(v) = prefs.edit().putInt("rotation", v).apply()
}
