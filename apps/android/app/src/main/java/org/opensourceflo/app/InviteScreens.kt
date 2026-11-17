package org.opensourceflo.app

import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.PersistableBundle
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.util.Locale

@Composable
internal fun OwnPasswordScreen(state: AuthState, model: AuthViewModel) {
    var current by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var confirm by remember { mutableStateOf("") }
    Notice(state)
    BodyText("Введите текущий пароль. После смены все сессии будут завершены, потребуется войти снова.")
    RetroField("Текущий пароль", current, !state.busy, password = true) { current = it }
    RetroField("Новый пароль", password, !state.busy, password = true) { password = it }
    RetroField("Повторите новый пароль", confirm, !state.busy, password = true) { confirm = it }
    if (confirm.isNotEmpty() && password != confirm) BodyText("Новые пароли не совпадают.", error = true)
    RetroButton("Изменить пароль", enabled = !state.busy && current.isNotEmpty() && validPassword(password) && password == confirm) {
        val old = current
        val new = password
        current = ""; password = ""; confirm = ""
        model.changeOwnPassword(old, new)
    }
}

@Composable
internal fun OwnerInviteScreen(state: AuthState, model: AuthViewModel, admin: Boolean = false) {
    val context = LocalContext.current
    var feedback by remember { mutableStateOf<String?>(null) }
    Notice(state)
    if (admin) BodyText("Создание аккаунта без пары. Доступны только владелец дневника и партнёр; ссылка одноразовая, на 24 часа.")
    else BodyText("Одноразовое приглашение действует 24 часа. Передайте его только выбранному партнёру. Новая ссылка отменяет предыдущую.")
    val invitation = state.generatedInvite
    if (invitation != null) RetroCard {
        Text(if (invitation.url == null) "КОД ПРИГЛАШЕНИЯ" else "ССЫЛКА ПРИГЛАШЕНИЯ", fontSize = 10.sp)
        SelectionContainer { Text(invitation.shareValue, fontSize = 13.sp, lineHeight = 20.sp) }
        BodyText("Действует до: ${invitation.expiresAt}")
        invitation.role?.let { BodyText("Роль: $it. Без пары.") }
        if (invitation.url == null) BodyText("Адрес приложения настроит администратор. Партнёр может вставить этот код в приложении.")
        RetroButton(if (invitation.url == null) "Скопировать код" else "Скопировать ссылку", enabled = !state.busy, secondary = true) {
            feedback = try {
                val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                val clip = ClipData.newPlainText("Приглашение партнёра", invitation.shareValue)
                if (Build.VERSION.SDK_INT >= 33) clip.description.extras = PersistableBundle().apply { putBoolean(ClipDescription.EXTRA_IS_SENSITIVE, true) }
                clipboard.setPrimaryClip(clip)
                "Приглашение скопировано."
            } catch (_: Exception) { "Не удалось скопировать приглашение." }
        }
        RetroButton("Поделиться", enabled = !state.busy, secondary = true) {
            feedback = try {
                val send = Intent(Intent.ACTION_SEND).apply { type = "text/plain"; putExtra(Intent.EXTRA_TEXT, invitation.shareValue) }
                context.startActivity(Intent.createChooser(send, "Передать приглашение"))
                null
            } catch (_: Exception) { "Не удалось открыть меню отправки." }
        }
    }
    feedback?.let { BodyText(it) }
    if (admin) {
        for (role in listOf(AccountRole.TRACKER, AccountRole.PARTNER)) {
            RetroButton("Создать ссылку: ${role.label}", enabled = !state.busy) { model.generateInvite(role) }
            RetroButton("Отозвать: ${role.label}", enabled = !state.busy, secondary = true) { model.revokeInvite(role) }
        }
        return
    }
    RetroButton(if (invitation == null) "Создать приглашение" else "Создать новое приглашение", enabled = !state.busy) { feedback = null; model.generateInvite() }
    RetroButton("Отозвать приглашения", enabled = !state.busy, secondary = true) { feedback = null; model.revokeInvite() }
}

@Composable
internal fun JoinInviteScreen(state: AuthState, model: AuthViewModel) {
    var input by remember { mutableStateOf("") }
    var mode by remember(state.pendingInvite) { mutableStateOf("choose") }
    Notice(state)
    val invitation = state.pendingInvite
    if (invitation == null) {
        BodyText("Вставьте одноразовую ссылку или код от владельца дневника или администратора. Предпросмотр не создаёт аккаунт и не принимает приглашение.")
        RetroField("Ссылка или код приглашения", input, !state.busy) { input = it }
        RetroButton("Проверить приглашение", enabled = !state.busy && input.isNotBlank()) {
            val submitted = input
            input = ""
            model.previewInvite(submitted)
        }
        return
    }
    RetroCard {
        Text("Приглашает: ${invitation.preview.trackerDisplayName}", fontSize = 18.sp, lineHeight = 26.sp, fontWeight = FontWeight.Bold)
        BodyText("Действует до: ${invitation.preview.expiresAt}")
        if (invitation.preview.kind == "account") BodyText("Создание аккаунта: ${invitation.preview.role.label}. Привязка к другому человеку не создаётся.")
        else BodyText("Принятие свяжет аккаунты. Партнёр сможет просматривать данные, которыми делится владелец дневника. Связь можно прекратить.")
    }
    when {
        state.user == null -> {
            when (mode) {
                "register" -> InviteRegistrationForm(state, model)
                "login" -> InviteLoginForm(state, model)
                else -> {
                    RetroButton("Создать аккаунт: ${invitation.preview.role.label}", enabled = !state.busy) { mode = "register" }
                    if (invitation.preview.kind != "account") RetroButton("У меня уже есть аккаунт", enabled = !state.busy, secondary = true) { mode = "login" }
                }
            }
            if (mode != "choose") RetroButton("Выбрать другой способ входа", enabled = !state.busy, secondary = true) { mode = "choose" }
        }
        invitation.preview.kind == "account" -> {
            BodyText("Эта ссылка создаёт новый аккаунт и не меняет текущий.")
            RetroButton("Выйти и создать аккаунт", enabled = !state.busy) { model.logout() }
        }
        state.user.role != AccountRole.PARTNER -> {
            BodyText("Приглашение предназначено аккаунту партнёра. Выйдите и войдите как партнёр или создайте новый аккаунт по приглашению.")
            RetroButton("Выйти и выбрать аккаунт партнёра", enabled = !state.busy) { model.logout() }
        }
        state.ownPair != null -> BodyText("Ваш аккаунт уже связан в пару. Чтобы принять новое приглашение, сначала прекратите существующую связь.")
        else -> {
            BodyText("Выполнен вход: ${state.user.displayName}. Вход сам по себе не принимает приглашение.")
            RetroButton("Подтвердить и принять приглашение", enabled = !state.busy) { model.acceptInvite() }
        }
    }
    RetroButton("Отменить приглашение", enabled = !state.busy, secondary = true) { model.cancelInvite() }
}

@Composable
private fun InviteLoginForm(state: AuthState, model: AuthViewModel) {
    BodyText("Войдите в существующий аккаунт партнёра. Затем отдельно подтвердите приглашение.")
    LoginForm(enabled = !state.busy, submitLabel = "Войти в аккаунт партнёра", onSubmit = model::login)
}

@Composable
private fun InviteRegistrationForm(state: AuthState, model: AuthViewModel) {
    var username by remember { mutableStateOf("") }
    var name by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var confirm by remember { mutableStateOf("") }
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        if (state.pendingInvite?.preview?.kind == "account") BodyText("Будет создан аккаунт: ${state.pendingInvite.preview.role.label}. Без пары.")
        else BodyText("Будет создан аккаунт партнёра и установлена связь с указанным владельцем дневника. Открытой регистрации нет.")
        RetroField("Логин", username, !state.busy) { username = it }
        BodyText("3–64 символа: a–z, 0–9, точка, дефис и подчёркивание.")
        RetroField("Отображаемое имя", name, !state.busy) { name = it }
        RetroField("Пароль", password, !state.busy, password = true) { password = it }
        RetroField("Повторите пароль", confirm, !state.busy, password = true) { confirm = it }
        if (confirm.isNotEmpty() && password != confirm) BodyText("Пароли не совпадают.", error = true)
        RetroButton("Создать аккаунт и принять приглашение", enabled = !state.busy &&
            username.trim().lowercase(Locale.ROOT).matches(Regex("[a-z0-9_.-]{3,64}")) &&
            name.trim().codePointCount(0, name.trim().length) in 1..80 && validPassword(password) && password == confirm
        ) {
            val submitted = password
            password = ""; confirm = ""
            model.registerInvitedPartner(username, name, submitted)
        }
    }
}


@Composable
internal fun DeleteAccountScreen(state: AuthState, model: AuthViewModel) {
    var open by remember { mutableStateOf(false) }
    var password by remember { mutableStateOf("") }
    Notice(state)
    if (!open) {
        RetroButton("Удалить мой аккаунт", enabled = !state.busy, secondary = true) { open = true }
    } else {
        BodyText("Аккаунт будет удалён навсегда вместе с сессиями, приглашениями и парой. Аккаунт партнёра сохранится. Последнего активного администратора удалить нельзя.")
        RetroField("Текущий пароль", password, !state.busy, password = true) { password = it }
        RetroButton("Да, удалить аккаунт навсегда", enabled = !state.busy && password.isNotEmpty()) {
            val value = password; password = ""; model.deleteSelf(value)
        }
        RetroButton("Отмена", enabled = !state.busy, secondary = true) { password = ""; open = false }
    }
}
