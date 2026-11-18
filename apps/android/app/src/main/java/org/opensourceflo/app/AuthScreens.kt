package org.opensourceflo.app

import java.util.Locale

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
internal fun LoginScreen(state: AuthState, model: AuthViewModel) {
    Notice(state)
    if (state.needsSetup) {
        BodyText("Первый администратор ещё не создан. Выполните команду на компьютере, где запущен сервер.")
        RetroCard {
            Text("В КОРНЕ ПРОЕКТА", fontSize = 10.sp)
            Text("./scripts/create-admin.sh", fontSize = 13.sp)
            BodyText("Пароль вводится скрыто в терминале. Аккаунт создаётся локально, без стандартного пароля.")
        }
        RetroButton("Проверить настройку", enabled = !state.busy) { model.reload() }
        return
    }
    BodyText("Введите логин и пароль, которые выдал администратор вашего сервера.")
    LoginForm(enabled = !state.busy, onSubmit = model::login)
    if (state.error != null && state.user == null) RetroButton("Проверить сервер", enabled = !state.busy, secondary = true) { model.reload() }
}

@Composable
internal fun ProfileScreen(state: AuthState, model: AuthViewModel) {
    val user = state.user ?: return
    Text(user.displayName, fontSize = 20.sp, lineHeight = 28.sp, fontWeight = FontWeight.Bold)
    BodyText("@${user.username} · ${user.role.label}")
    if (user.role != AccountRole.ADMIN) Notice(state)
    if (user.role != AccountRole.ADMIN) {
        Text("Связанный аккаунт", fontSize = 16.sp, fontWeight = FontWeight.Bold)
        val pair = state.ownPair
        if (pair == null) BodyText("Активной пары нет. Администратор может связать аккаунты.") else {
            val other = if (user.id == pair.tracker.id) pair.partner else pair.tracker
            RetroCard {
                Text(other.displayName, fontSize = 18.sp)
                BodyText("@${other.username} · ${other.role.label}")
                ConfirmUnlink("Прекратить связь", "Прекратить связь с ${other.displayName}? Новые данные больше не будут доступны партнёру.", state.busy) { model.unlinkOwnPair() }
            }
        }
    }
    RetroButton("Обновить", enabled = !state.busy, secondary = true) { model.reload() }
    RetroButton("Выйти", enabled = !state.busy) { model.logout() }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun AdminScreen(state: AuthState, model: AuthViewModel) {
    var tab by remember { mutableStateOf("users") }
    Notice(state)
    FlowRow(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        RetroButton("Аккаунты", enabled = !state.busy, secondary = tab != "users", fillWidth = false, modifier = Modifier.semantics { selected = tab == "users" }) { tab = "users" }
        RetroButton("Пары", enabled = !state.busy, secondary = tab != "pairs", fillWidth = false, modifier = Modifier.semantics { selected = tab == "pairs" }) { tab = "pairs" }
    }
    if (tab == "users") {
        CreateUserForm(state, model)
        Text("Аккаунты · ${state.users.size}", fontWeight = FontWeight.Bold, fontSize = 18.sp)
        state.users.forEach { user -> key(user.id) { UserCard(user, state, model) } }
    } else {
        CreatePairForm(state, model)
        Text("Пары · ${state.pairs.size}", fontWeight = FontWeight.Bold, fontSize = 18.sp)
        if (state.pairs.isEmpty()) BodyText("Активных пар нет.")
        state.pairs.forEach { pair -> key(pair.id) {
            RetroCard {
                Text(pair.tracker.displayName + " ↔ " + pair.partner.displayName, fontSize = 17.sp, lineHeight = 25.sp)
                BodyText("@${pair.tracker.username} · @${pair.partner.username}")
                BodyText("Создана: ${pair.createdAt}")
                ConfirmUnlink("Разорвать пару", "Прекратить связь? Новые данные больше не будут доступны партнёру.", state.busy) { model.unlinkPair(pair) }
            }
        } }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun CreateUserForm(state: AuthState, model: AuthViewModel) {
    var username by remember { mutableStateOf("") }
    var name by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var role by remember { mutableStateOf(AccountRole.TRACKER) }
    LaunchedEffect(state.createdUserVersion) { username = ""; name = ""; password = "" }
    RetroCard {
        Text("Создать аккаунт", fontSize = 18.sp, fontWeight = FontWeight.Bold)
        RetroField("Логин", username, !state.busy) { username = it }
        BodyText("3–64 символа: a–z, 0–9, точка, дефис и подчёркивание.")
        RetroField("Отображаемое имя", name, !state.busy) { name = it }
        RetroField("Пароль", password, !state.busy, password = true) { password = it }
        Text("Роль", fontSize = 12.sp)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            AccountRole.entries.forEach { option ->
                RetroButton(option.label, enabled = !state.busy, secondary = role != option, fillWidth = false, modifier = Modifier.semantics { selected = role == option }) { role = option }
            }
        }
        RetroButton("Создать аккаунт", enabled = !state.busy && username.trim().lowercase(Locale.ROOT).matches(Regex("[a-z0-9_.-]{3,64}")) && name.trim().codePointCount(0, name.trim().length) in 1..80 && validPassword(password)) {
            val submitted = password
            password = ""
            model.createUser(username, name, submitted, role)
        }
    }
}

@Composable
private fun UserCard(user: Account, state: AuthState, model: AuthViewModel) {
    var resetOpen by remember { mutableStateOf(false) }
    var deleteOpen by remember { mutableStateOf(false) }
    var deactivateOpen by remember { mutableStateOf(false) }
    var password by remember { mutableStateOf("") }
    val own = user.id == state.user?.id
    RetroCard {
        Text(user.displayName, fontSize = 18.sp, fontWeight = FontWeight.Bold)
        BodyText("@${user.username} · ${user.role.label}")
        BodyText(if (user.active) "Активен" else "Отключён")
        if (!own) {
            if (deactivateOpen) {
                BodyText("Отключить ${user.displayName}? Активные сессии будут завершены.")
                RetroButton("Да, отключить", enabled = !state.busy) { deactivateOpen = false; model.setActive(user, false) }
                RetroButton("Отмена", enabled = !state.busy, secondary = true) { deactivateOpen = false }
            } else RetroButton(if (user.active) "Отключить" else "Включить", enabled = !state.busy, secondary = true) {
                if (user.active) deactivateOpen = true else model.setActive(user, true)
            }
        } else BodyText("Ваш текущий аккаунт нельзя отключить здесь.")
        if (!own) {
            if (deleteOpen) {
                BodyText("Удалить аккаунт навсегда? Сессии и пара будут удалены, аккаунт партнёра сохранится.")
                RetroButton("Да, удалить навсегда", enabled = !state.busy) { deleteOpen = false; model.deleteUser(user) }
                RetroButton("Отмена", enabled = !state.busy, secondary = true) { deleteOpen = false }
            } else RetroButton("Удалить аккаунт", enabled = !state.busy, secondary = true) { deleteOpen = true }
        }
        if (resetOpen) {
            RetroField("Новый пароль", password, !state.busy, password = true) { password = it }
            BodyText("Смена пароля завершит все сессии этого аккаунта.")
            RetroButton("Сохранить пароль", enabled = !state.busy && validPassword(password)) {
                val submitted = password
                password = ""
                resetOpen = false
                model.resetPassword(user, submitted)
            }
            RetroButton("Отмена", enabled = !state.busy, secondary = true) { password = ""; resetOpen = false }
        } else RetroButton("Сбросить пароль", enabled = !state.busy, secondary = true) { resetOpen = true }
    }
}

@Composable
private fun CreatePairForm(state: AuthState, model: AuthViewModel) {
    var trackerId by remember { mutableStateOf("") }
    var partnerId by remember { mutableStateOf("") }
    LaunchedEffect(state.createdPairVersion) { trackerId = ""; partnerId = "" }
    val pairedIds = state.pairs.flatMap { listOf(it.tracker.id, it.partner.id) }.toSet()
    val available = state.users.filter { it.active && it.id !in pairedIds }
    val trackers = available.filter { it.role == AccountRole.TRACKER }
    val partners = available.filter { it.role == AccountRole.PARTNER }
    RetroCard {
        Text("Связать аккаунты", fontSize = 18.sp, fontWeight = FontWeight.Bold)
        AccountPicker("Владелец дневника", trackers, trackerId, !state.busy) { trackerId = it }
        AccountPicker("Партнёр", partners, partnerId, !state.busy) { partnerId = it }
        BodyText("Доступны активные аккаунты без пары. Участники смогут прекратить связь.")
        RetroButton("Связать аккаунты", enabled = !state.busy && trackers.any { it.id == trackerId } && partners.any { it.id == partnerId }) { model.createPair(trackerId, partnerId) }
    }
}

@Composable
private fun AccountPicker(label: String, options: List<Account>, selectedId: String, enabled: Boolean, onSelect: (String) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Text(label, fontSize = 12.sp)
    Box {
        val selectedUser = options.firstOrNull { it.id == selectedId }
        RetroButton(selectedUser?.let { "${it.displayName} (@${it.username})" } ?: if (options.isEmpty()) "Нет доступных аккаунтов" else "Выберите аккаунт", enabled = enabled && options.isNotEmpty(), secondary = true) { open = true }
        DropdownMenu(expanded = open && enabled, onDismissRequest = { open = false }, containerColor = Panel) {
            options.forEach { account -> DropdownMenuItem(text = { Text("${account.displayName} (@${account.username})") }, onClick = { onSelect(account.id); open = false }) }
        }
    }
}

@Composable
private fun ConfirmUnlink(label: String, question: String, busy: Boolean, onConfirm: () -> Unit) {
    var confirming by remember { mutableStateOf(false) }
    if (confirming) {
        BodyText(question)
        RetroButton("Да, прекратить связь", enabled = !busy) { confirming = false; onConfirm() }
        RetroButton("Отмена", enabled = !busy, secondary = true) { confirming = false }
    } else RetroButton(label, enabled = !busy, secondary = true) { confirming = true }
}

internal fun validPassword(value: String): Boolean = value.codePointCount(0, value.length) in 12..128 && value.toByteArray(Charsets.UTF_8).size <= 512
