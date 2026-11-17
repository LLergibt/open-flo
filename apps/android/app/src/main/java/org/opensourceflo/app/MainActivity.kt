package org.opensourceflo.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.*
import androidx.compose.ui.text.PlatformTextStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel

internal val Ink = Color(0xFF303647)
internal val Paper = Color(0xFFF8E8D8)
internal val Panel = Color(0xFFFFF9F1)
internal val Accent = Color(0xFFF5D0B0)
internal val ErrorInk = Color(0xFF842E3D)

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            MaterialTheme(colorScheme = lightColorScheme(primary = Ink, background = Paper, surface = Panel, onBackground = Ink, onSurface = Ink)) {
                CompositionLocalProvider(LocalTextStyle provides TextStyle(
                    color = Ink, fontFamily = FontFamily.Monospace,
                    platformStyle = PlatformTextStyle(includeFontPadding = false)
                )) { AuthApplication() }
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun AuthApplication(model: AuthViewModel = viewModel()) {
    val state by model.state.collectAsStateWithLifecycle()
    Surface(color = Paper, modifier = Modifier.fillMaxSize()) {
        BoxWithConstraints(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing).imePadding()) {
            val mobile = maxWidth <= 480.dp
            Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()), horizontalAlignment = Alignment.CenterHorizontally) {
                Column(Modifier.widthIn(max = 760.dp).fillMaxWidth().padding(
                    start = if (mobile) 18.dp else 24.dp, end = if (mobile) 22.dp else 24.dp, top = 28.dp, bottom = 24.dp
                ), verticalArrangement = Arrangement.spacedBy(24.dp)) {
                    FlowRow(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        Text("flo / local", fontSize = 19.sp, fontWeight = FontWeight.Bold, letterSpacing = (-1).sp, modifier = Modifier.padding(end = 12.dp))
                        Text("АВТОРИЗАЦИЯ · 001", modifier = Modifier.border(1.dp, Ink).padding(7.dp, 5.dp), fontSize = 9.sp)
                    }
                    if (state.user == null) {
                        RetroWindow("auth.exe", if (state.needsSetup) "Настройка сервера" else "Вход в аккаунт") { LoginScreen(state, model) }
                    } else {
                        RetroWindow("profile.exe", "Мой аккаунт") { ProfileScreen(state, model) }
                        if (state.user?.role == AccountRole.ADMIN) RetroWindow("admin.exe", "Администрирование") { AdminScreen(state, model) }
                    }
                    if (state.user?.role == AccountRole.ADMIN) RetroWindow("invites.exe", "Ссылки регистрации") { OwnerInviteScreen(state, model, admin = true) }
                    if (state.user?.role == AccountRole.TRACKER && state.ownPair == null) {
                        RetroWindow("invite.exe", "Пригласить партнёра") { OwnerInviteScreen(state, model) }
                    }
                    if (!state.needsSetup && (state.user == null || state.user?.role == AccountRole.PARTNER && state.ownPair == null || state.pendingInvite != null)) {
                        RetroWindow("invite.exe", "Приглашение партнёра") { JoinInviteScreen(state, model) }
                    }
                    if (state.user != null) RetroWindow("password.exe", "Смена пароля") { OwnPasswordScreen(state, model) }
                    if (state.user != null) RetroWindow("account.exe", "Удаление аккаунта") { DeleteAccountScreen(state, model) }
                    Text(
                        "Токены хранятся только в памяти. После перезапуска приложения потребуется войти снова.",
                        modifier = Modifier.widthIn(max = 470.dp), fontSize = 11.sp, lineHeight = 18.7.sp
                    )
                }
            }
        }
    }
}

@Composable
internal fun RetroWindow(name: String, title: String, content: @Composable ColumnScope.() -> Unit) {
    Column(Modifier.fillMaxWidth().drawBehind {
        val offset = 5.dp.toPx()
        drawRect(Ink, topLeft = Offset(offset, offset), size = size)
    }.background(Panel).border(2.dp, Ink).padding(2.dp)) {
        Row(Modifier.fillMaxWidth().background(Accent).padding(10.dp, 7.dp).clearAndSetSemantics {}, horizontalArrangement = Arrangement.SpaceBetween) {
            Text(name, fontSize = 12.sp, lineHeight = 15.sp)
            Text("─ □ ×", fontSize = 12.sp, fontWeight = FontWeight.Bold, letterSpacing = 4.sp)
        }
        HorizontalDivider(thickness = 2.dp, color = Ink)
        Column(Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 26.dp), verticalArrangement = Arrangement.spacedBy(18.dp)) {
            Text("СВОЙ СЕРВЕР · СВОИ ДАННЫЕ", fontSize = 10.sp, letterSpacing = 1.sp)
            Text(title, modifier = Modifier.semantics { heading() }, fontSize = 25.sp, lineHeight = 30.sp, letterSpacing = (-1).sp, fontWeight = FontWeight.Bold)
            content()
        }
        HorizontalDivider(thickness = 1.dp, color = Ink)
        Row(Modifier.fillMaxWidth().padding(12.dp, 10.dp), horizontalArrangement = Arrangement.SpaceBetween) {
            Text("open-source-flo", fontSize = 10.sp)
            Text("android / dev", fontSize = 10.sp)
        }
    }
}

@Composable
internal fun BodyText(text: String, error: Boolean = false) {
    Text(text, fontFamily = FontFamily.SansSerif, fontSize = 14.sp, lineHeight = 22.sp, color = if (error) ErrorInk else Ink)
}

@Composable
internal fun Notice(state: AuthState) {
    state.error?.let { Text(it, color = ErrorInk, fontFamily = FontFamily.SansSerif, fontSize = 14.sp, lineHeight = 22.sp, modifier = Modifier.semantics { liveRegion = LiveRegionMode.Assertive }) }
    state.notice?.let { Text(it, fontFamily = FontFamily.SansSerif, fontSize = 14.sp, lineHeight = 22.sp, modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }) }
    if (state.busy) Text("Загрузка…", fontSize = 12.sp, modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite })
}

@Composable
internal fun RetroButton(text: String, modifier: Modifier = Modifier, enabled: Boolean = true, secondary: Boolean = false, fillWidth: Boolean = true, onClick: () -> Unit) {
    Button(
        onClick = onClick, enabled = enabled, modifier = modifier.then(if (fillWidth) Modifier.fillMaxWidth() else Modifier).heightIn(min = 48.dp), shape = RectangleShape,
        border = BorderStroke(2.dp, Ink), contentPadding = PaddingValues(14.dp, 11.dp),
        colors = ButtonDefaults.buttonColors(containerColor = if (secondary) Accent else Ink, contentColor = if (secondary) Ink else Panel,
            disabledContainerColor = Ink.copy(alpha = 0.45f), disabledContentColor = Panel)
    ) { Text(text, color = if (secondary) Ink else Panel, fontFamily = FontFamily.Monospace, fontSize = 13.sp, lineHeight = 19.sp, fontWeight = FontWeight.Normal) }
}

@Composable
internal fun RetroCard(content: @Composable ColumnScope.() -> Unit) {
    Column(Modifier.fillMaxWidth().background(Paper).border(1.dp, Ink).padding(17.dp), verticalArrangement = Arrangement.spacedBy(12.dp), content = content)
}
