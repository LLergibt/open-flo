package org.opensourceflo.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.*
import androidx.compose.ui.unit.dp

@Composable
internal fun LoginForm(enabled: Boolean, submitLabel: String = "Войти", onSubmit: (String, String) -> Unit) {
    var username by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        RetroField("Логин", username, enabled) { username = it }
        RetroField("Пароль", password, enabled, password = true) { password = it }
        RetroButton(submitLabel, enabled = enabled && username.isNotBlank() && password.isNotEmpty()) {
            val submitted = password
            password = ""
            onSubmit(username, submitted)
        }
    }
}
