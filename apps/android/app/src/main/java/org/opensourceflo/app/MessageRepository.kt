package org.opensourceflo.app

import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject

data class DatabaseMessage(val message: String, val source: String)

class MessageRepository(private val baseUrl: String = BuildConfig.API_BASE_URL) {
    suspend fun load(): DatabaseMessage = withContext(Dispatchers.IO) {
        val connection = URL("${baseUrl.trimEnd('/')}/api/message").openConnection() as HttpURLConnection
        try {
            connection.requestMethod = "GET"
            connection.connectTimeout = 8_000
            connection.readTimeout = 8_000
            connection.instanceFollowRedirects = false
            connection.setRequestProperty("Accept", "application/json")
            if (connection.responseCode != HttpURLConnection.HTTP_OK) {
                throw IOException("Unexpected server response")
            }
            val body = connection.inputStream.bufferedReader(Charsets.UTF_8).use { reader ->
                val output = StringBuilder()
                val buffer = CharArray(4096)
                while (true) {
                    val count = reader.read(buffer)
                    if (count < 0) break
                    if (output.length + count > 65_536) throw IOException("Response too large")
                    output.append(buffer, 0, count)
                }
                output.toString()
            }
            val json = JSONObject(body)
            val message = json.opt("message") as? String ?: throw IOException("Invalid message response")
            val source = json.opt("source") as? String ?: throw IOException("Invalid message response")
            if (message.isBlank() || source != "sqlite") throw IOException("Invalid message response")
            DatabaseMessage(message, source)
        } finally {
            connection.disconnect()
        }
    }
}
