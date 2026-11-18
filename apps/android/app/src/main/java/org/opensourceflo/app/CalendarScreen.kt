package org.opensourceflo.app

import android.app.DatePickerDialog
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Checkbox
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import java.time.LocalDate
import java.time.YearMonth
import java.time.format.DateTimeFormatter
import java.util.Locale

private val russian = Locale.forLanguageTag("ru")
private val dayFormat = DateTimeFormatter.ofPattern("d MMMM yyyy", russian)
private val monthFormat = DateTimeFormatter.ofPattern("LLLL yyyy", russian)
private val painLabels = listOf("Не отмечена", "Слабая", "Умеренная", "Сильная")
private val moods = listOf("" to "Не отмечено", "calm" to "Спокойствие", "happy" to "Радость", "sad" to "Грусть",
    "anxious" to "Тревога", "irritable" to "Раздражительность", "energetic" to "Энергичность")
private val flows = listOf("" to "Не отмечено", "spotting" to "Мажущие выделения", "light" to "Слабые", "medium" to "Умеренные", "heavy" to "Обильные")
private val contexts = listOf("regular" to "Регулярный цикл", "irregular" to "Нерегулярный цикл", "pregnancy" to "Беременность",
    "postpartum" to "После родов", "hormonal_contraception" to "Гормональная контрацепция", "paused" to "Приостановить прогноз")

@Composable
internal fun CalendarScreen(state: AuthState, model: AuthViewModel) {
    val account = state.user ?: return
    val data = state.calendar
    val context = LocalContext.current
    val catalog = remember(context) {
        context.assets.open("diary-catalog.json").bufferedReader().use { parseDiaryCatalog(it.readText()) }
    }
    var selected by remember(account.id) { mutableStateOf(LocalDate.now()) }
    var month by remember(account.id) { mutableStateOf(YearMonth.from(selected)) }
    var showing by remember(account.id) { mutableStateOf(false) }
    LaunchedEffect(data == null) { if (data == null) showing = false }
    if (data == null) {
        BodyText(state.calendarError ?: "Загружаем календарь…")
        RetroButton("Обновить", enabled = !state.busy, secondary = true) { model.refreshCalendar() }
        return
    }
    if (!data.owner) BodyText("Календарь и дневник · ${data.name}. Только просмотр.")
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
        RetroButton("←", enabled = month > YearMonth.of(2000, 1) && !state.busy, secondary = true, fillWidth = false,
            modifier = Modifier.semantics { contentDescription = "Предыдущий месяц" }) { month = month.minusMonths(1) }
        Text(month.format(monthFormat).replaceFirstChar { it.uppercase(russian) }, fontWeight = FontWeight.Bold, fontSize = 14.sp)
        RetroButton("→", enabled = month < YearMonth.of(2100, 12) && !state.busy, secondary = true, fillWidth = false,
            modifier = Modifier.semantics { contentDescription = "Следующий месяц" }) { month = month.plusMonths(1) }
    }
    MonthGrid(month, selected, data, !state.busy) {
        selected = it
        month = YearMonth.from(it)
        showing = true
    }
    BodyText("● Месячные · факт   ┄ Прогноз   ~ Окно   ○ Овуляция\n≈ / ◌ Прошлые циклы   · Запись дневника")
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        RetroButton("Сегодня", enabled = !state.busy, secondary = true, fillWidth = false) {
            selected = data.today; month = YearMonth.from(selected); showing = true
        }
        RetroButton("Обновить", enabled = !state.busy, secondary = true, fillWidth = false) { model.refreshCalendar() }
    }
    ForecastCard(data)
    if (data.owner) {
        OwnerCycleControls(data, selected, state, model)
        BodyText("Связанный партнёр видит календарь и сохранённые записи.")
    }
    Notice(state)
    if (showing) key(selected) {
        DaySheet(data, selected, catalog, state, model) { showing = false }
    }
}

@Composable
private fun MonthGrid(month: YearMonth, selected: LocalDate, data: CalendarData, enabled: Boolean, select: (LocalDate) -> Unit) {
    val first = month.atDay(1)
    val leading = first.dayOfWeek.value - 1
    val firstCell = first.minusDays(leading.toLong())
    val weeks = (leading + month.lengthOfMonth() + 6) / 7
    Row(Modifier.fillMaxWidth()) {
        for (label in listOf("Пн","Вт","Ср","Чт","Пт","Сб","Вс")) {
            Box(Modifier.weight(1f), contentAlignment = Alignment.Center) { Text(label, fontSize = 11.sp) }
        }
    }
    repeat(weeks) { week ->
        Row(Modifier.fillMaxWidth()) {
            repeat(7) { column ->
                val day = firstCell.plusDays((week * 7 + column).toLong())
                val kind = data.kind(day)
                val background = when (kind) {
                    DayKind.PERIOD -> Color(0xFFF2B8BB)
                    DayKind.FORECAST -> Color(0xFFFFF2F0)
                    DayKind.NONE -> if (month.month == day.month) Panel else Paper
                    else -> Color(0xFFDCE9F8)
                }
                val logged = day in data.diary
                val current = day == data.today
                val label = listOf(day.format(dayFormat), if (current) "сегодня" else "", kind.label,
                    if (logged) "есть запись дневника" else "").filter(String::isNotEmpty).joinToString(", ")
                Column(Modifier.weight(1f).heightIn(min = 48.dp).background(background)
                    .border(if (day == selected) 2.dp else 0.5.dp, Ink)
                    .semantics { contentDescription = label; this.selected = day == selected }
                    .clickable(enabled && day.year in 2000..2100, role = Role.Button) { select(day) },
                    horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
                    Text("${day.dayOfMonth}${if (logged) "·" else ""}", fontWeight = if (current) FontWeight.Bold else FontWeight.Normal, fontSize = 13.sp)
                    Text(kind.symbol.ifEmpty { " " }, fontSize = 11.sp)
                }
            }
        }
    }
}

@Composable
private fun ForecastCard(data: CalendarData) {
    RetroCard {
        Text(if (data.status == "overdue") "Ожидаемая дата прошла" else "Календарный прогноз", fontWeight = FontWeight.Bold)
        data.expected?.let { BodyText("Месячные: ${it.start.format(dayFormat)} — ${it.end.format(dayFormat)}") }
        val current = data.forecasts.firstOrNull()
        current?.ovulation?.let { BodyText("Предполагаемая овуляция: ${it.format(dayFormat)}") }
        current?.fertile?.let { BodyText("Расчётное окно: ${it.start.format(dayFormat)} — ${it.end.format(dayFormat)}") }
        when (data.status) {
            "insufficient_data" -> BodyText("Добавьте дату начала последних месячных.")
            "paused" -> BodyText("Прогноз приостановлен.")
            "irregular" -> BodyText("Для оценки овуляции и окна сейчас недостаточно данных.")
            "overdue" -> BodyText("Если месячные начались, отметьте фактическую дату.")
        }
        if (data.source == "baseline") BodyText("Стартовая оценка использует условные 28 дней.")
        BodyText("Календарь не подходит для контрацепции.")
    }
}

@Composable
private fun DaySheet(data: CalendarData, date: LocalDate, catalog: List<DiaryCategory>, state: AuthState, model: AuthViewModel, close: () -> Unit) {
    val entry = data.diary[date]
    var editing by remember { mutableStateOf(false) }
    var draft by remember(entry) { mutableStateOf(entry ?: DiaryDay()) }
    var confirmDelete by remember { mutableStateOf(false) }
    LaunchedEffect(state.notice) { if (state.notice == "Изменения сохранены.") editing = false }
    Dialog(onDismissRequest = close, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Column(Modifier.fillMaxSize().imePadding().safeDrawingPadding().padding(top = 30.dp),
            verticalArrangement = Arrangement.Bottom) {
            Column(Modifier.fillMaxWidth().fillMaxHeight(.86f).background(Panel).border(2.dp, Ink)) {
                Row(Modifier.fillMaxWidth().background(Accent).padding(horizontal = 14.dp, vertical = 6.dp),
                    horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                    Text("Подробности дня", fontSize = 15.sp, fontWeight = FontWeight.Bold)
                    RetroButton("Закрыть", secondary = true, fillWidth = false, onClick = close)
                }
                Column(Modifier.fillMaxWidth().weight(1f).verticalScroll(rememberScrollState()).padding(14.dp),
                    verticalArrangement = Arrangement.spacedBy(11.dp)) {
                    Text(date.format(dayFormat), fontSize = 20.sp, fontWeight = FontWeight.Bold)
                    data.kind(date).label.takeIf(String::isNotEmpty)?.let { BodyText(it) }
                    Notice(state)
                    if (!editing) {
                        DiarySummary(entry, catalog)
                        if (data.canEdit(date)) RetroButton(if (entry == null) "Добавить запись" else "Редактировать запись", enabled = !state.busy) {
                            draft = entry ?: DiaryDay(); editing = true
                        }
                    } else {
                        RetroButton("Назад к подробностям", enabled = !state.busy, secondary = true) { editing = false; confirmDelete = false }
                        DiaryFields(draft, catalog, !state.busy) { draft = it }
                        RetroButton("Сохранить запись", enabled = !state.busy) { model.saveDiary(date, draft) }
                        if (entry != null && !confirmDelete) RetroButton("Удалить запись дня", enabled = !state.busy, secondary = true) { confirmDelete = true }
                        if (confirmDelete) {
                            BodyText("Удалить запись за этот день?")
                            RetroButton("Да, удалить запись", enabled = !state.busy) { model.deleteDiary(date) }
                            RetroButton("Отмена", enabled = !state.busy, secondary = true) { confirmDelete = false }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun DiarySummary(entry: DiaryDay?, catalog: List<DiaryCategory>) {
    if (entry == null) { BodyText("За этот день записей нет."); return }
    BodyText("Боль: ${painLabels.getOrElse(entry.pain) { "Не отмечена" }}")
    BodyText("Интенсивность кровотечения: ${flows.toMap()[entry.flow]}")
    BodyText("Настроение: ${moods.toMap()[entry.mood]}")
    for (group in catalog) {
        val selected = if (group.symptom) entry.symptoms else entry.events
        val labels = group.options.filter { it.first in selected }.joinToString(", ") { it.second }
        if (labels.isNotBlank()) BodyText("${group.title}: $labels")
    }
    if (entry.notes.isNotBlank()) {
        Text("Заметка", fontWeight = FontWeight.Bold)
        BodyText(entry.notes)
    }
}

@Composable
private fun DiaryFields(entry: DiaryDay, catalog: List<DiaryCategory>, enabled: Boolean, update: (DiaryDay) -> Unit) {
    ChoicePanel("Боль", painLabels.mapIndexed { index, label -> index.toString() to label }, entry.pain.toString(), enabled) { update(entry.copy(pain = it.toInt())) }
    ChoicePanel("Интенсивность кровотечения", flows, entry.flow, enabled) { update(entry.copy(flow = it)) }
    ChoicePanel("Настроение", moods, entry.mood, enabled) { update(entry.copy(mood = it)) }
    for (group in catalog) key(group.title) {
        val values = if (group.symptom) entry.symptoms else entry.events
        ExpandPanel(if (group.options.any { it.first in values }) "${group.title} ·" else group.title, enabled) {
            group.options.forEach { (code, label) ->
                CheckLine(label, code in values, enabled) { checked ->
                    val next = if (checked) values + code else values - code
                    update(if (group.symptom) entry.copy(symptoms = next) else entry.copy(events = next))
                }
            }
        }
    }
    androidx.compose.material3.OutlinedTextField(value = entry.notes,
        onValueChange = { if (it.codePointCount(0, it.length) <= 2000) update(entry.copy(notes = it)) },
        enabled = enabled, modifier = Modifier.fillMaxWidth(), label = { Text("Личная заметка") }, minLines = 3)
}

@Composable
private fun CheckLine(label: String, checked: Boolean, enabled: Boolean, change: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth().heightIn(min = 48.dp).clickable(enabled, role = Role.Checkbox) { change(!checked) },
        verticalAlignment = Alignment.CenterVertically) {
        Checkbox(checked = checked, onCheckedChange = null, enabled = enabled, modifier = Modifier.padding(end = 10.dp))
        Text(label, fontSize = 13.sp)
    }
}

@Composable
private fun ChoicePanel(title: String, options: List<Pair<String, String>>, value: String, enabled: Boolean, choose: (String) -> Unit) {
    ExpandPanel("$title · ${options.toMap()[value] ?: ""}", enabled) {
        options.forEach { (id, label) -> CheckLine(label, id == value, enabled) { choose(id) } }
    }
}

@Composable
private fun ExpandPanel(title: String, enabled: Boolean, body: @Composable ColumnScope.() -> Unit) {
    var open by remember { mutableStateOf(false) }
    HorizontalDivider(color = Ink)
    RetroButton("${if (open) "−" else "+"} $title", enabled = enabled, secondary = true) { open = !open }
    if (open) Column(verticalArrangement = Arrangement.spacedBy(8.dp), content = body)
}

@Composable
private fun OwnerCycleControls(data: CalendarData, selected: LocalDate, state: AuthState, model: AuthViewModel) {
    ExpandPanel("Месячные и история дат", !state.busy) {
        var editing by remember { mutableStateOf<PeriodRecord?>(null) }
        var start by remember { mutableStateOf(minOf(data.today, selected)) }
        var end by remember { mutableStateOf<LocalDate?>(null) }
        var deleting by remember { mutableStateOf<String?>(null) }
        DateButton("Дата начала", start, data.today, !state.busy) { start = it; if (end?.isBefore(it) == true) end = null }
        CheckLine("Месячные ещё идут", end == null, !state.busy) { end = if (it) null else start }
        end?.let { DateButton("Дата окончания", it, data.today, !state.busy) { day -> if (day >= start) end = day } }
        RetroButton(if (editing == null) "Добавить месячные" else "Сохранить исправление", enabled = !state.busy) { model.savePeriod(editing?.id, start, end) }
        if (editing != null) RetroButton("Отмена редактирования", enabled = !state.busy, secondary = true) {
            editing = null; start = minOf(data.today, selected); end = null
        }
        for (period in data.periods.asReversed()) key(period.id) {
            BodyText("${period.start.format(dayFormat)} — ${period.end?.format(dayFormat) ?: "продолжаются"}")
            RetroButton("Изменить даты", enabled = !state.busy, secondary = true) { editing = period; start = period.start; end = period.end }
            RetroButton(if (deleting == period.id) "Подтвердить удаление" else "Удалить запись", enabled = !state.busy, secondary = true) {
                if (deleting == period.id) { model.deletePeriod(period.id); deleting = null } else deleting = period.id
            }
        }
    }
    val settings = data.settings ?: return
    ExpandPanel("Настройки расчёта", !state.busy) {
        var length by remember(settings) { mutableStateOf(settings.optInt("cycle_length").takeIf { it > 0 }?.toString().orEmpty()) }
        var duration by remember(settings) { mutableStateOf(settings.optInt("period_length", 5).toString()) }
        var method by remember(settings) { mutableStateOf(settings.optString("length_method", "auto")) }
        var mode by remember(settings) { mutableStateOf(settings.optString("context", "regular")) }
        RetroField("Привычная длина цикла, дней", length, !state.busy) { length = it.filter(Char::isDigit).take(2) }
        RetroField("Длительность месячных, дней", duration, !state.busy) { duration = it.filter(Char::isDigit).take(2) }
        ChoicePanel("Способ расчёта", listOf("auto" to "По истории", "manual" to "Использовать моё число"), method, !state.busy) { method = it }
        ChoicePanel("Режим цикла", contexts, mode, !state.busy) { mode = it }
        val parsed = length.toIntOrNull()
        val days = duration.toIntOrNull()
        RetroButton("Сохранить настройки", enabled = !state.busy && days != null && days in 1..14 &&
            (length.isEmpty() && method == "auto" || parsed != null && parsed in 10..90)) {
            model.saveCycleSettings(parsed, days ?: return@RetroButton, method, mode)
        }
    }
}

@Composable
private fun DateButton(label: String, date: LocalDate, today: LocalDate, enabled: Boolean, select: (LocalDate) -> Unit) {
    val context = LocalContext.current
    RetroButton("$label · ${date.format(dayFormat)}", enabled = enabled, secondary = true) {
        DatePickerDialog(context, { _, year, month, day -> select(LocalDate.of(year, month + 1, day)) },
            date.year, date.monthValue - 1, date.dayOfMonth).apply {
            datePicker.maxDate = today.atStartOfDay(java.time.ZoneId.systemDefault()).toInstant().toEpochMilli()
            datePicker.minDate = LocalDate.of(2000, 1, 1).atStartOfDay(java.time.ZoneId.systemDefault()).toInstant().toEpochMilli()
        }.show()
    }
}
