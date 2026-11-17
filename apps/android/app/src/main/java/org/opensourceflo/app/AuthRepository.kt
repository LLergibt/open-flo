package org.opensourceflo.app

import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.util.Locale
import java.util.concurrent.ScheduledThreadPoolExecutor
import java.util.concurrent.ThreadFactory
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import org.json.JSONObject

enum class AccountRole(val wire: String, val label: String) {
    ADMIN("admin", "Администратор"), TRACKER("tracker", "Владелец дневника"), PARTNER("partner", "Партнёр");
    companion object {
        fun parse(value: String): AccountRole = entries.firstOrNull { it.wire == value } ?: throw ApiFailure("invalid_response")
    }
}
data class Account(val id: String, val username: String, val displayName: String, val role: AccountRole, val active: Boolean)
data class AccountPair(val id: String, val tracker: Account, val partner: Account, val createdAt: String)
class ApiFailure(val code: String) : Exception(code)
internal class NativeSession(val user: Account, val access: String, val refresh: String)
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
                val result = StringBuilder()
                val buffer = CharArray(4096)
                while (true) {
                    currentCoroutineContext().ensureActive()
                    if (System.nanoTime() > deadline) throw IOException("Request deadline")
                    val count = reader.read(buffer)
                    if (count < 0) break
                    if (result.length + count > 1_048_576) throw IOException("Response size limit")
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

class AuthRepository internal constructor(private val transport: ApiTransport) {
    constructor() : this(HttpApiTransport())
    private val sessionMutex = Mutex()
    private var session: NativeSession? = null
    private var generation = 0L

    suspend fun needsSetup(): Boolean = json(expect(transport.send("GET", "/api/setup/status", null, null), 200)).getBoolean("needs_setup")

    suspend fun login(username: String, password: String): Account {
        val attempt = sessionMutex.withLock { session = null; ++generation }
        val body = JSONObject().put("username", username.trim().lowercase(Locale.ROOT)).put("password", password).put("client", "native")
        val response = expect(transport.send("POST", "/api/auth/login", null, body.toString()), 200)
        val loggedIn = decodeSession(response)
        sessionMutex.withLock {
            if (generation != attempt) throw ApiFailure("unauthorized")
            session = loggedIn
        }
        return loggedIn.user
    }

    suspend fun clearSession() = sessionMutex.withLock { session = null; generation++ }

    suspend fun logout() {
        val previous = sessionMutex.withLock { val old = session; session = null; generation++; old } ?: return
        val body = JSONObject().put("refresh_token", previous.refresh).put("client", "native").toString()
        expect(transport.send("POST", "/api/auth/logout", null, body), 204)
    }

    private suspend fun protected(method: String, path: String, body: JSONObject? = null): ApiResponse {
        val snapshot = sessionMutex.withLock { (session ?: throw ApiFailure("unauthorized")) to generation }
        val original = snapshot.first
        var response = transport.send(method, path, original.access, body?.toString())
        if (response.status != 401 || errorCode(response) != "unauthorized") return response
        val renewed = sessionMutex.withLock {
            val current = session ?: throw ApiFailure("unauthorized")
            if (generation != snapshot.second) throw ApiFailure("unauthorized")
            if (current !== original) current else {
                try {
                    val payload = JSONObject().put("refresh_token", current.refresh).put("client", "native")
                    val refreshed = decodeSession(expect(transport.send("POST", "/api/auth/refresh", null, payload.toString()), 200))
                    if (refreshed.user.id != current.user.id) throw ApiFailure("invalid_response")
                    session = refreshed
                    refreshed
                } catch (cancelled: CancellationException) {
                    // The server may already have rotated the token; never replay it after cancellation.
                    session = null
                    generation++
                    throw cancelled
                } catch (_: Exception) {
                    session = null
                    generation++
                    throw ApiFailure("unauthorized")
                }
            }
        }
        response = transport.send(method, path, renewed.access, body?.toString())
        if (response.status == 401 && errorCode(response) == "unauthorized") {
            sessionMutex.withLock { if (session === renewed) { session = null; generation++ } }
            throw ApiFailure("unauthorized")
        }
        return response
    }

    suspend fun me(): Account = account(json(expect(protected("GET", "/api/me"), 200)))
    suspend fun ownPair(): AccountPair? {
        val value = json(expect(protected("GET", "/api/me/pair"), 200))
        if (!value.has("pair")) throw ApiFailure("invalid_response")
        return if (value.isNull("pair")) null else pair(value.getJSONObject("pair"))
    }
    suspend fun unlinkOwnPair() { expect(protected("DELETE", "/api/me/pair"), 204) }
    suspend fun users(): List<Account> {
        val values = json(expect(protected("GET", "/api/admin/users"), 200)).getJSONArray("users")
        return List(values.length()) { account(values.getJSONObject(it)) }
    }
    suspend fun pairs(): List<AccountPair> {
        val values = json(expect(protected("GET", "/api/admin/pairs"), 200)).getJSONArray("pairs")
        return List(values.length()) { pair(values.getJSONObject(it)) }
    }
    suspend fun createUser(username: String, displayName: String, password: String, role: AccountRole): Account {
        val body = JSONObject().put("username", username.trim().lowercase(Locale.ROOT)).put("display_name", displayName.trim())
            .put("password", password).put("role", role.wire)
        return account(json(expect(protected("POST", "/api/admin/users", body), 201)))
    }
    suspend fun setActive(id: String, active: Boolean): Account = account(json(expect(
        protected("PATCH", "/api/admin/users/${safeId(id)}", JSONObject().put("is_active", active)), 200
    )))
    suspend fun resetPassword(id: String, password: String): Account = account(json(expect(
        protected("PATCH", "/api/admin/users/${safeId(id)}", JSONObject().put("password", password)), 200
    )))
    suspend fun createPair(trackerId: String, partnerId: String): AccountPair = pair(json(expect(
        protected("POST", "/api/admin/pairs", JSONObject().put("tracker_id", trackerId).put("partner_id", partnerId)), 201
    )))
    suspend fun unlinkPair(id: String) { expect(protected("DELETE", "/api/admin/pairs/${safeId(id)}"), 204) }

    suspend fun changeOwnPassword(currentPassword: String, newPassword: String) {
        expect(protected("POST", "/api/me/password", JSONObject().put("current_password", currentPassword).put("new_password", newPassword)), 204)
        clearSession()
    }
    suspend fun generateInvite(): GeneratedInvitation {
        val value = json(expect(protected("POST", "/api/me/invite", JSONObject()), 201))
        val token = string(value, "token")
        if (InviteInput.parse(token) != token || !value.has("url")) throw ApiFailure("invalid_response")
        val url = if (value.isNull("url")) null else string(value, "url")
        if (url != null && InviteInput.parse(url) != token) throw ApiFailure("invalid_response")
        return GeneratedInvitation(token, url, string(value, "expires_at"))
    }
    suspend fun revokeInvite() { expect(protected("DELETE", "/api/me/invite"), 204) }
    suspend fun previewInvite(token: String): InvitePreview {
        val value = json(expect(transport.send("POST", "/api/invites/preview", null, JSONObject().put("token", token).toString()), 200))
        return InvitePreview(string(value, "tracker_display_name"), string(value, "expires_at"))
    }
    suspend fun registerInvite(token: String, username: String, name: String, password: String): InviteRegistration {
        val body = JSONObject().put("token", token).put("username", username.trim().lowercase(Locale.ROOT))
            .put("display_name", name.trim()).put("password", password).put("client", "native")
        val value = json(expect(transport.send("POST", "/api/invites/register", null, body.toString()), 201))
        val user = account(value.getJSONObject("user"))
        val pair = pair(value.getJSONObject("pair"))
        if (user.role != AccountRole.PARTNER || pair.partner.id != user.id) throw ApiFailure("invalid_response")
        return InviteRegistration(user, pair)
    }
    suspend fun acceptInvite(token: String): AccountPair = pair(json(expect(
        protected("POST", "/api/me/invite/accept", JSONObject().put("token", token)), 201
    )))

    private fun errorCode(response: ApiResponse): String = runCatching { JSONObject(response.body).optString("error") }.getOrDefault("")
    private fun safeId(id: String): String {
        if (!id.matches(Regex("[a-fA-F0-9-]{36}"))) throw ApiFailure("invalid_response")
        return id
    }
    private fun json(response: ApiResponse): JSONObject = try { JSONObject(response.body) } catch (_: Exception) { throw ApiFailure("invalid_response") }
    private fun expect(response: ApiResponse, expected: Int): ApiResponse {
        if (response.status == expected) return response
        val allowed = setOf("unauthorized", "forbidden", "invalid_input", "invalid_credentials", "conflict", "rate_limited", "internal_error", "invite_unavailable")
        val error = runCatching { JSONObject(response.body).optString("error") }.getOrDefault("")
        throw ApiFailure(if (error in allowed) error else when (response.status) {
            401 -> "unauthorized"; 403 -> "forbidden"; 400 -> "invalid_input"; 409 -> "conflict"; 429 -> "rate_limited"; else -> "internal_error"
        })
    }
    private fun string(value: JSONObject, name: String): String = (value.opt(name) as? String)?.takeIf { it.isNotBlank() } ?: throw ApiFailure("invalid_response")
    private fun account(value: JSONObject): Account = try {
        Account(string(value, "id"), string(value, "username"), string(value, "display_name"), AccountRole.parse(string(value, "role")), value.getBoolean("is_active"))
    } catch (_: Exception) { throw ApiFailure("invalid_response") }
    private fun pair(value: JSONObject): AccountPair = try {
        AccountPair(string(value, "id"), account(value.getJSONObject("tracker")), account(value.getJSONObject("partner")), string(value, "created_at"))
    } catch (_: Exception) { throw ApiFailure("invalid_response") }
    private fun decodeSession(response: ApiResponse): NativeSession {
        val value = json(response)
        val user = account(value.getJSONObject("user"))
        if (!user.active) throw ApiFailure("unauthorized")
        val access = string(value, "access_token")
        val refresh = string(value, "refresh_token")
        if (access.length > 8192 || refresh.length > 4096 || access.any { it == '\r' || it == '\n' }) throw ApiFailure("invalid_response")
        return NativeSession(user, access, refresh)
    }
}

fun authErrorMessage(code: String): String = when (code) {
    "unauthorized" -> "Сессия завершена. Войдите снова."
    "forbidden" -> "Недостаточно прав для этого действия."
    "invalid_input" -> "Проверьте заполнение полей. Пароль — от 12 до 128 символов."
    "invalid_credentials" -> "Неверный логин или пароль."
    "current_password_incorrect" -> "Текущий пароль неверен."
    "invalid_invite" -> "Вставьте ссылку приглашения или код из 64 шестнадцатеричных символов."
    "invite_unavailable" -> "Приглашение недоступно: срок истёк, ссылка отозвана или уже использована."
    "conflict" -> "Аккаунт уже существует или один из участников уже связан в пару."
    "rate_limited" -> "Слишком много попыток. Попробуйте позже."
    "network_error" -> "Не удалось связаться с API. Проверьте подключение и адрес сервера."
    "invalid_response" -> "Сервер вернул некорректный ответ. Повторите попытку."
    else -> "Сервер временно недоступен. Повторите попытку."
}
