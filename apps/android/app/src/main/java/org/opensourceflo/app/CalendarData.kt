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
    val isEmpty: Boolean
        get() = symptoms.isEmpty() && events.isEmpty() && mood.isBlank() && pain == 0 && notes.isBlank()

    fun json(includeFlow: Boolean = true) = JSONObject().put("symptoms", JSONArray(symptoms.toList()))
        .put("events", JSONArray(events.toList())).put("mood", mood).put("pain", pain).put("notes", notes)
        .apply { if (includeFlow) put("flow", flow) }
}

internal data class DiaryCategory(
    val title: String, val symptom: Boolean, val options: List<Pair<String, String>>,
    val ownerOnly: Boolean = false, val ownerOnlyOptions: Set<String> = emptySet()
)

internal fun partnerCategories(catalog: List<DiaryCategory>): List<DiaryCategory> =
    catalog.filterNot(DiaryCategory::ownerOnly).map { category ->
        category.copy(options = category.options.filterNot { (id) -> id in category.ownerOnlyOptions })
    }

internal data class CalendarData(
    val owner: Boolean, val name: String, val today: LocalDate, val periods: List<PeriodRecord>,
    val diary: Map<LocalDate, DiaryDay>, val forecasts: List<ForecastBand>, val expected: DateBand?,
    val status: String, val source: String?, val settings: JSONObject?,
    val partnerDiary: Map<LocalDate, DiaryDay> = emptyMap(), val partnerDiaryEnabled: Boolean = false,
    val partnerPairId: String? = null, val viewerId: String? = null
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
    fun canEdit(day: LocalDate) = (owner || partnerPairId != null) && day <= today
}

private fun JSONObject.date(key: String): LocalDate? = if (isNull(key)) null else LocalDate.parse(getString(key))
private fun JSONObject.band(first: String, last: String): DateBand? {
    val start = date(first) ?: return null
    val end = date(last) ?: return null
    require(start <= end)
    return DateBand(start, end)
}
private fun <T> JSONArray.mapItems(transform: (Int) -> T): List<T> = List(length(), transform)

private fun JSONArray.stringSet(): Set<String> = mapItems(::getString).toSet()

private fun JSONArray.diaryDays(owner: Boolean): Map<LocalDate, DiaryDay> = mapItems {
    val row = getJSONObject(it)
    row.date("date")!! to DiaryDay(
        symptoms = if (owner || row.has("symptoms")) row.getJSONArray("symptoms").stringSet() else emptySet(),
        events = if (owner || row.has("events")) row.getJSONArray("events").stringSet() else emptySet(),
        mood = row.getString("mood"),
        flow = if (owner) row.getString("flow") else "",
        pain = if (owner || row.has("pain")) row.getInt("pain") else 0,
        notes = row.getString("notes")
    )
}.toMap()

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
    val partnerEntries = value.optJSONArray("partner_entries") ?: JSONArray()
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
        entries.diaryDays(owner = true), forecasts, expected, status,
        prediction.optString("source").takeUnless { it.isEmpty() || it == "null" },
        value.optJSONObject("settings").takeIf { owner },
        partnerEntries.diaryDays(owner = false), value.optBoolean("partner_diary_enabled"),
        value.optString("partner_pair_id").takeIf { !value.isNull("partner_pair_id") && it.isNotBlank() },
        value.optString("viewer_id").takeIf { !value.isNull("viewer_id") && it.isNotBlank() })
}
internal fun parseDiaryCatalog(text: String): List<DiaryCategory> {
    val groups = JSONArray(text)
    return groups.mapItems { index ->
        val item = groups.getJSONObject(index)
        val options = item.getJSONArray("options")
        DiaryCategory(item.getString("title"), item.getString("field") == "symptoms", options.mapItems {
            val option = options.getJSONArray(it)
            option.getString(0) to option.getString(1)
        }, item.optBoolean("owner_only"), item.optJSONArray("owner_only_options")?.stringSet().orEmpty())
    }
}
