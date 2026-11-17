package org.opensourceflo.app

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.*
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.semantics.*
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
internal fun RetroField(label: String, value: String, enabled: Boolean, password: Boolean = false, onChange: (String) -> Unit) {
    var visible by remember { mutableStateOf(false) }
    LaunchedEffect(value) { if (value.isEmpty()) visible = false }
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(label, fontSize = 12.sp)
        OutlinedTextField(
            value = value, onValueChange = onChange, enabled = enabled, modifier = Modifier.fillMaxWidth().semantics { contentDescription = label },
            singleLine = true, shape = RectangleShape,
            textStyle = TextStyle(fontFamily = FontFamily.SansSerif, fontSize = 16.sp, color = Ink),
            visualTransformation = if (password && !visible) PasswordVisualTransformation() else VisualTransformation.None,
            keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, keyboardType = if (password) KeyboardType.Password else KeyboardType.Text),
            colors = OutlinedTextFieldDefaults.colors(focusedBorderColor = Ink, unfocusedBorderColor = Ink, focusedContainerColor = Paper, unfocusedContainerColor = Paper),
            trailingIcon = if (password) ({
                IconButton(onClick = { visible = !visible }, enabled = enabled,
                    modifier = Modifier.size(48.dp).semantics { contentDescription = "${if (visible) "Скрыть" else "Показать"} пароль: $label"; stateDescription = if (visible) "Пароль виден" else "Пароль скрыт" }) {
                    Canvas(Modifier.size(24.dp)) {
                        val stroke = Stroke(width = 1.8.dp.toPx())
                        drawOval(Ink, topLeft = Offset(1.dp.toPx(), 5.dp.toPx()), size = Size(22.dp.toPx(), 14.dp.toPx()), style = stroke)
                        drawCircle(Ink, radius = 3.dp.toPx(), style = stroke)
                        if (visible) drawLine(Ink, Offset(3.dp.toPx(), 3.dp.toPx()), Offset(21.dp.toPx(), 21.dp.toPx()), strokeWidth = stroke.width)
                    }
                }
            }) else null,
            supportingText = if (password) ({ Text("От 12 до 128 символов", fontSize = 11.sp) }) else null
        )
    }
}

