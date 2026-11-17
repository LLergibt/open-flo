package org.opensourceflo.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.PlatformTextStyle
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel

private val Ink = Color(0xFF303647)
private val Paper = Color(0xFFF8E8D8)
private val Panel = Color(0xFFFFF9F1)
private val Accent = Color(0xFFF5D0B0)
private val ErrorInk = Color(0xFF842E3D)

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            MaterialTheme(colorScheme = lightColorScheme(primary = Ink, background = Paper, surface = Panel, onBackground = Ink, onSurface = Ink)) {
                CompositionLocalProvider(LocalTextStyle provides TextStyle(
                    color = Ink,
                    fontFamily = FontFamily.Monospace,
                    platformStyle = PlatformTextStyle(includeFontPadding = false)
                )) {
                    MessageScreen()
                }
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun MessageScreen(model: MessageViewModel = viewModel()) {
    val state by model.state.collectAsStateWithLifecycle()
    Surface(color = Paper, modifier = Modifier.fillMaxSize()) {
        BoxWithConstraints(modifier = Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
            val mobile = maxWidth <= 480.dp
            val headingSize = (maxWidth.value * 0.05f).coerceIn(25f, 36f).sp
            Column(
                modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()),
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                Column(modifier = Modifier.widthIn(max = 760.dp).fillMaxWidth().padding(
                    start = if (mobile) 18.dp else 24.dp,
                    end = if (mobile) 22.dp else 24.dp,
                    top = 24.dp,
                    bottom = 24.dp
                )) {
                    Spacer(Modifier.height(if (mobile) 4.dp else 12.dp))
                    FlowRow(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalArrangement = Arrangement.spacedBy(10.dp)
                    ) {
                        Text(
                            buildAnnotatedString {
                                withStyle(SpanStyle(fontWeight = FontWeight(750))) { append("flo") }
                                append(" / ")
                                withStyle(SpanStyle(fontWeight = FontWeight(750))) { append("local") }
                            },
                            modifier = Modifier.padding(end = if (mobile) 12.dp else 16.dp),
                            fontSize = if (mobile) 19.sp else 22.sp,
                            lineHeight = if (mobile) 23.sp else 26.sp,
                            letterSpacing = (-1).sp
                        )
                        Text(
                            "КАРКАС · 001",
                            modifier = Modifier.border(1.dp, Ink).padding(horizontal = 7.dp, vertical = 5.dp),
                            fontSize = if (mobile) 9.sp else 10.sp,
                            lineHeight = if (mobile) 11.sp else 12.sp
                        )
                    }
                    Spacer(Modifier.height(if (mobile) 32.dp else 52.dp))
                    Column(modifier = Modifier.fillMaxWidth()
                        .drawBehind {
                            val offset = 5.dp.toPx()
                            drawRect(color = Ink, topLeft = Offset(offset, offset), size = size)
                        }
                        .background(Panel).border(2.dp, Ink).padding(2.dp)
                    ) {
                        Row(
                            modifier = Modifier.fillMaxWidth().background(Accent)
                                .padding(horizontal = 10.dp, vertical = 7.dp).clearAndSetSemantics {},
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.SpaceBetween
                        ) {
                            Text("connection.exe", fontSize = 12.sp, lineHeight = 15.sp)
                            Text("─ □ ×", fontSize = 12.sp, lineHeight = 15.sp, fontWeight = FontWeight.Bold, letterSpacing = 4.sp)
                        }
                        HorizontalDivider(thickness = 2.dp, color = Ink)
                        Column(modifier = Modifier.fillMaxWidth().padding(
                            start = if (mobile) 20.dp else 32.dp,
                            end = if (mobile) 20.dp else 32.dp,
                            top = if (mobile) 26.dp else 38.dp,
                            bottom = if (mobile) 24.dp else 32.dp
                        )) {
                            Text("СВОЙ СЕРВЕР · СВОИ ДАННЫЕ", fontSize = 10.sp, lineHeight = 12.sp, letterSpacing = 1.sp)
                            Spacer(Modifier.height(16.dp))
                            Text(
                                "Проверка подключения",
                                modifier = Modifier.semantics { heading() },
                                fontSize = headingSize,
                                lineHeight = headingSize * 1.2f,
                                fontWeight = FontWeight(750),
                                letterSpacing = if (mobile) (-1).sp else (-1.7).sp
                            )
                            Spacer(Modifier.height(18.dp))
                            Text(
                                "Эта страница получает тестовое сообщение из базы данных через API.",
                                modifier = Modifier.widthIn(max = 470.dp),
                                fontFamily = FontFamily.SansSerif,
                                fontSize = if (mobile) 16.sp else 17.sp,
                                lineHeight = if (mobile) 26.4.sp else 28.05.sp
                            )
                            Spacer(Modifier.height(30.dp))
                            FlowRow(
                                modifier = Modifier.semantics(mergeDescendants = true) {
                                    contentDescription = "Путь получения данных: SQLite, API, Приложение"
                                },
                                horizontalArrangement = Arrangement.spacedBy(10.dp),
                                verticalArrangement = Arrangement.spacedBy(6.dp)
                            ) {
                                Text("SQLite", fontSize = 12.sp, lineHeight = 15.sp)
                                Text("→  API", fontSize = 12.sp, lineHeight = 15.sp)
                                Text("→  Приложение", fontSize = 12.sp, lineHeight = 15.sp)
                            }
                            Spacer(Modifier.height(22.dp))
                            MessagePanel(state, mobile)
                            Spacer(Modifier.height(24.dp))
                            if (mobile) {
                                Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                                    RefreshButton(state, model::refresh, Modifier.fillMaxWidth())
                                    ConnectionStatus(state)
                                }
                            } else {
                                FlowRow(
                                    horizontalArrangement = Arrangement.spacedBy(20.dp),
                                    verticalArrangement = Arrangement.spacedBy(16.dp),
                                    itemVerticalAlignment = Alignment.CenterVertically
                                ) {
                                    RefreshButton(state, model::refresh)
                                    ConnectionStatus(state)
                                }
                            }
                        }
                        HorizontalDivider(thickness = 1.dp, color = Ink)
                        FlowRow(
                            modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 10.dp),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalArrangement = Arrangement.spacedBy(6.dp)
                        ) {
                            Text("open-source-flo", fontSize = 10.sp, lineHeight = 12.sp, modifier = Modifier.padding(end = 12.dp))
                            Text("android / dev", fontSize = 10.sp, lineHeight = 12.sp)
                        }
                    }
                    Spacer(Modifier.height(24.dp))
                    Text(
                        "Тестовая сборка. Здесь используются только демонстрационные данные.",
                        modifier = Modifier.widthIn(max = 470.dp),
                        fontSize = 11.sp,
                        lineHeight = 18.7.sp
                    )
                }
            }
        }
    }
}

@Composable
private fun MessagePanel(state: MessageState, mobile: Boolean) {
    Column(modifier = Modifier.fillMaxWidth().heightIn(min = 168.dp).background(Paper)
        .border(1.dp, Ink).padding(1.dp).padding(if (mobile) 17.dp else 20.dp)
    ) {
        Text("СООБЩЕНИЕ ИЗ БАЗЫ", fontSize = 10.sp, lineHeight = 12.sp, letterSpacing = 0.7.sp)
        Spacer(Modifier.height(19.dp))
        Column(modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }) {
            Text(
                when (state) {
                    MessageState.Loading -> "Получаем сообщение…"
                    is MessageState.Loaded -> state.value.message
                    MessageState.Failed -> "Соединение не установлено"
                },
                fontSize = if (mobile) 18.sp else 20.sp,
                lineHeight = if (mobile) 27.sp else 30.sp,
                fontWeight = if (state is MessageState.Loading) FontWeight.Normal else FontWeight(650)
            )
            Spacer(Modifier.height(12.dp))
            Text(
                when (state) {
                    MessageState.Loading -> "Подключаемся к серверу."
                    is MessageState.Loaded -> "Источник: ${state.value.source} · GET /api/message"
                    MessageState.Failed -> "Не удалось связаться с API. Проверьте, что сервер запущен."
                },
                color = if (state is MessageState.Failed) ErrorInk else Ink,
                fontFamily = FontFamily.SansSerif,
                fontSize = 13.sp,
                lineHeight = 19.5.sp
            )
        }
    }
}

@Composable
private fun RefreshButton(state: MessageState, refresh: () -> Unit, modifier: Modifier = Modifier) {
    val loading = state is MessageState.Loading
    val interactions = remember { MutableInteractionSource() }
    val pressed by interactions.collectIsPressedAsState()
    val hovered by interactions.collectIsHoveredAsState()
    val highlighted = !loading && (pressed || hovered)
    Button(
        onClick = refresh,
        enabled = !loading,
        modifier = modifier.heightIn(min = 48.dp).graphicsLayer {
            alpha = if (loading) 0.6f else 1f
            translationX = if (pressed) 1.dp.toPx() else 0f
            translationY = if (pressed) 1.dp.toPx() else 0f
        },
        shape = RectangleShape,
        border = BorderStroke(2.dp, Ink),
        colors = ButtonDefaults.buttonColors(
            containerColor = if (highlighted) Accent else Ink,
            contentColor = if (highlighted) Ink else Panel,
            disabledContainerColor = Ink,
            disabledContentColor = Panel
        ),
        contentPadding = PaddingValues(horizontal = 17.dp, vertical = 11.dp),
        interactionSource = interactions
    ) {
        Text(
            when (state) {
                MessageState.Loading -> "Загрузка…"
                MessageState.Failed -> "Повторить подключение"
                is MessageState.Loaded -> "Получить снова"
            },
            color = if (highlighted) Ink else Panel,
            fontFamily = FontFamily.Monospace,
            fontWeight = FontWeight.Normal,
            fontSize = 13.sp,
            lineHeight = 18.2.sp
        )
    }
}

@Composable
private fun ConnectionStatus(state: MessageState) {
    Row(
        modifier = Modifier.semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite },
        horizontalArrangement = Arrangement.spacedBy(7.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            when (state) {
                MessageState.Loading -> "○"
                MessageState.Failed -> "×"
                is MessageState.Loaded -> "●"
            },
            modifier = Modifier.clearAndSetSemantics {},
            fontSize = 18.sp,
            lineHeight = 18.sp
        )
        Text(
            when (state) {
                MessageState.Loading -> "Ожидание"
                MessageState.Failed -> "Нет соединения"
                is MessageState.Loaded -> "Подключено"
            },
            fontSize = 11.sp,
            lineHeight = 14.sp
        )
    }
}
