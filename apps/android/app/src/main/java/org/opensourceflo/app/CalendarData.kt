package org.opensourceflo.app

import java.time.LocalDate
import org.json.JSONArray
import org.json.JSONObject

internal data class DateBand(val start: LocalDate, val end: LocalDate) {
    operator fun contains(day: LocalDate) = day >= start && day <= end
}
internal data class PeriodRecord(val id: String, val start: LocalDate, val end: LocalDate?)
internal data class ForecastBand(val period: DateBand?, val fertile: DateBand?, val ovulation: LocalDate?, val historical: Boolean = false)
internal enum class DayKind(val label: String, val symbol: String) {
    PERIOD("Отмеченные месячные", "●"), FORECAST("Ожидаемые месячные", "┄"),
    FERTILE("Расчётное фертильное окно", "~"), OVULATION("Предполагаемая овуляция", "○"),
    PAST_FERTILE("Оценка окна прошлого цикла", "≈"), PAST_OVULATION("Оценка овуляции прошлого цикла", "◌"),
    NONE("", "")
}
internal data class DiaryDay(
    val symptoms: Set<String> = emptySet(), val events: Set<String> = emptySet(),
    val mood: String = "", val flow: String = "", val pain: Int = 0, val notes: String = ""
) {
    fun json() = JSONObject().put("symptoms", JSONArray(symptoms.toList())).put("events", JSONArray(events.toList()))
        .put("mood", mood).put("flow", flow).put("pain", pain).put("notes", notes)
}
internal data class DiaryCategory(val title: String, val symptom: Boolean, val options: List<Pair<String, String>>)
internal data class CalendarData(
    val owner: Boolean, val name: String, val today: LocalDate, val periods: List<PeriodRecord>,
    val diary: Map<LocalDate, DiaryDay>, val forecasts: List<ForecastBand>, val expected: DateBand?,
    val status: String, val source: String?, val settings: JSONObject?
) {
    fun kind(day: LocalDate): DayKind {
        if (periods.any { day in DateBand(it.start, it.end ?: today) }) return DayKind.PERIOD
        if (forecasts.any { it.period?.contains(day) == true }) return DayKind.FORECAST
        val ovulation = forecasts.firstOrNull { it.ovulation == day }
        if (ovulation != null) return if (ovulation.historical) DayKind.PAST_OVULATION else DayKind.OVULATION
        val fertile = forecasts.firstOrNull { it.fertile?.contains(day) == true }
        return when (fertile?.historical) {
            true -> DayKind.PAST_FERTILE
            false -> DayKind.FERTILE
            null -> DayKind.NONE
        }
    }
    fun canEdit(day: LocalDate) = owner && day <= today
}

private fun JSONObject.date(key: String): LocalDate? = if (isNull(key)) null else LocalDate.parse(getString(key))
private fun JSONObject.band(first: String, last: String): DateBand? {
    val start = date(first) ?: return null
    val end = date(last) ?: return null
    require(start <= end)
    return DateBand(start, end)
}
private fun <T> JSONArray.mapItems(transform: (Int) -> T): List<T> = List(length(), transform)

internal fun parseCalendar(value: JSONObject): CalendarData {
    val prediction = value.getJSONObject("prediction")
    val status = prediction.getString("status")
    val expected = prediction.band("next_period_start", "next_period_end")
    val forecasts = buildList {
        add(ForecastBand(expected.takeUnless { status == "overdue" }, prediction.band("fertile_start", "fertile_end"), prediction.date("ovulation_date")))
        val extra = prediction.optJSONArray("additional_cycles") ?: JSONArray()
        addAll(extra.mapItems {
            val item = extra.getJSONObject(it)
            ForecastBand(item.band("period_start", "period_end"), item.band("fertile_start", "fertile_end"),
                item.date("ovulation_date"), item.getString("kind") == "retrospective")
        })
    }
    val periods = value.getJSONArray("periods")
    val entries = value.getJSONArray("entries")
    val owner = when (value.getString("access")) {
        "owner" -> true
        "partner" -> false
        else -> throw ApiFailure("invalid_response")
    }
    return CalendarData(owner, value.getString("owner_display_name"), value.date("as_of")!!,
        periods.mapItems {
            val row = periods.getJSONObject(it)
            PeriodRecord(row.getString("id"), row.date("start_date")!!, row.date("end_date"))
        },
        entries.mapItems {
            val row = entries.getJSONObject(it)
            val symptoms = row.getJSONArray("symptoms")
            val events = row.getJSONArray("events")
            row.date("date")!! to DiaryDay(symptoms.mapItems(symptoms::getString).toSet(),
                events.mapItems(events::getString).toSet(), row.getString("mood"), row.getString("flow"),
                row.getInt("pain"), row.getString("notes"))
        }.toMap(), forecasts, expected, status,
        prediction.optString("source").takeUnless { it.isEmpty() || it == "null" },
        value.optJSONObject("settings").takeIf { owner })
}
internal fun parseDiaryCatalog(text: String): List<DiaryCategory> {
    val groups = JSONArray(text)
    return groups.mapItems { index ->
        val item = groups.getJSONObject(index)
        val options = item.getJSONArray("options")
        DiaryCategory(item.getString("title"), item.getString("field") == "symptoms", options.mapItems {
            val option = options.getJSONArray(it)
            option.getString(0) to option.getString(1)
        })
    }
}
