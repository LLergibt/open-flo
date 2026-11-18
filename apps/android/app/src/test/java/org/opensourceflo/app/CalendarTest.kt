package org.opensourceflo.app

import java.time.LocalDate
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test

class CalendarTest {
    private val first = LocalDate.parse("2026-11-01")

    @Test fun selectsSavedDiaryAndGivesPeriodVisualPriority() {
        val snapshot = parseCalendar(JSONObject("""
            {"access":"partner","owner_display_name":"Fixture","as_of":"2026-11-18","settings":null,
            "periods":[{"id":"fixture","start_date":"2026-11-01","end_date":"2026-11-03"}],
            "entries":[{"date":"2026-11-01","symptoms":["acne"],"events":["orgasm"],"mood":"","flow":"","pain":1,"notes":"fixture"}],
            "prediction":{"status":"estimated","source":"history","next_period_start":"2026-11-28","next_period_end":"2026-12-02",
            "ovulation_date":"2026-11-14","fertile_start":"2026-11-03","fertile_end":"2026-11-18","additional_cycles":[
              {"kind":"projected","period_start":"2026-12-25","period_end":"2026-12-29","ovulation_date":null,"fertile_start":null,"fertile_end":null}]}}
        """.trimIndent()))
        assertEquals(setOf("acne"), snapshot.diary[first]?.symptoms)
        assertEquals(setOf("orgasm"), snapshot.diary[first]?.events)
        assertNull(snapshot.diary[first.plusDays(1)])
        assertEquals(DayKind.PERIOD, snapshot.kind(first.plusDays(2)))
        assertEquals(DayKind.FERTILE, snapshot.kind(first.plusDays(3)))
        assertEquals(DayKind.FORECAST, snapshot.kind(LocalDate.parse("2026-12-25")))
        assertFalse(snapshot.canEdit(first))
        assertTrue(snapshot.copy(owner = true).canEdit(first))
        assertFalse(snapshot.copy(owner = true).canEdit(LocalDate.parse("2026-11-19")))
    }

    @Test fun encodedDiaryKeepsMultipleCategoriesAndUnicode() {
        val entry = DiaryDay(setOf("acne", "cramps"), setOf("orgasm", "lubricant"), notes = "Строка\nи ещё одна")
        val json = entry.json()
        assertEquals(2, json.getJSONArray("symptoms").length())
        assertEquals(2, json.getJSONArray("events").length())
        assertEquals(entry.notes, json.getString("notes"))
    }
}
