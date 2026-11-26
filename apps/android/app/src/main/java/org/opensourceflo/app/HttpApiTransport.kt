package org.opensourceflo.app

import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.util.concurrent.ScheduledThreadPoolExecutor
import java.util.concurrent.ThreadFactory
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.withContext

internal data class ApiResponse(val status: Int, val body: String)
internal fun interface ApiTransport {
    suspend fun send(method: String, path: String, bearer: String?, body: String?): ApiResponse
}

internal class HttpApiTransport(private val baseUrl: String = BuildConfig.API_BASE_URL) : ApiTransport {
    companion object {
        private val deadlines = ScheduledThreadPoolExecutor(1, ThreadFactory { task ->
            Thread(task, "flo-http-deadline").apply { isDaemon = true }
        }).apply { removeOnCancelPolicy = true }
    }
    override suspend fun send(method: String, path: String, bearer: String?, body: String?): ApiResponse = withContext(Dispatchers.IO) {
        val connection = URL("${baseUrl.trimEnd('/')}$path").openConnection() as HttpURLConnection
        val deadline = System.nanoTime() + 15_000_000_000L
        val timeout = deadlines.schedule({ connection.disconnect() }, 15, TimeUnit.SECONDS)
        try {
            connection.requestMethod = method
            connection.instanceFollowRedirects = false
            connection.useCaches = false
            connection.connectTimeout = 8_000
            connection.readTimeout = 8_000
            connection.setRequestProperty("Accept", "application/json")
            if (path == "/api/calendar" || path.startsWith("/api/cycle/") || path.startsWith("/api/diary/") || path.startsWith("/api/partner-diary/")) {
                connection.setRequestProperty("X-Calendar-Date", java.time.LocalDate.now().toString())
            }
            if (bearer != null) connection.setRequestProperty("Authorization", "Bearer $bearer")
            if (body != null) {
                val bytes = body.toByteArray(Charsets.UTF_8)
                connection.doOutput = true
                connection.setRequestProperty("Content-Type", "application/json")
                connection.setFixedLengthStreamingMode(bytes.size)
                connection.outputStream.use { it.write(bytes) }
            }
            currentCoroutineContext().ensureActive()
            val status = connection.responseCode
            val stream = if (status in 200..299) connection.inputStream else connection.errorStream
            val text = stream?.bufferedReader(Charsets.UTF_8)?.use { reader ->
                val limit = if (path == "/api/calendar") 16_777_216 else 1_048_576
                val result = StringBuilder()
                val buffer = CharArray(4096)
                while (true) {
                    currentCoroutineContext().ensureActive()
                    if (System.nanoTime() > deadline) throw IOException("Request deadline")
                    val count = reader.read(buffer)
                    if (count < 0) break
                    if (result.length + count > limit) throw IOException("Response size limit")
                    result.append(buffer, 0, count)
                }
                result.toString()
            }.orEmpty()
            ApiResponse(status, text)
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (_: IOException) {
            throw ApiFailure("network_error")
        } finally {
            timeout.cancel(false)
            connection.disconnect()
        }
    }
}

