package org.opensourceflo.app

import java.util.Locale
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.withContext
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import org.json.JSONObject

class AuthRepository internal constructor(
    private val transport: ApiTransport,
    private val store: SessionStore? = null
) {
    private val sessionMutex = Mutex()
    private var session: NativeSession? = null
    private var generation = 0L

    suspend fun needsSetup(): Boolean = json(expect(transport.send("GET", "/api/setup/status", null, null), 200)).getBoolean("needs_setup")

    internal suspend fun restoreSession(): Boolean = sessionMutex.withLock {
        if (session == null) session = store?.load()
        session != null
    }

    suspend fun login(username: String, password: String): Account {
        val attempt = sessionMutex.withLock {
            clearSessionLocked()
            generation
        }
        val body = JSONObject().put("username", username.trim().lowercase(Locale.ROOT)).put("password", password).put("client", "native")
        val response = expect(transport.send("POST", "/api/auth/login", null, body.toString()), 200)
        val (account, loggedIn) = decodeSession(response)
        sessionMutex.withLock {
            if (generation != attempt) throw ApiFailure("unauthorized")
            withContext(NonCancellable) { saveSession(loggedIn) }
        }
        return account
    }

    suspend fun clearSession() = sessionMutex.withLock { clearSessionLocked() }

    private suspend fun clearSessionLocked() = withContext(NonCancellable) {
        session = null
        generation++
        store?.clear()
    }

    private suspend fun saveSession(next: NativeSession) {
        store?.save(next)
        session = next
    }

    suspend fun logout() {
        val previous = sessionMutex.withLock {
            val current = session
            clearSessionLocked()
            current
        } ?: return
        val body = JSONObject().put("refresh_token", previous.refresh).put("client", "native").toString()
        expect(transport.send("POST", "/api/auth/logout", null, body), 204)
    }

    private suspend fun refreshSession(current: NativeSession): NativeSession = withContext(NonCancellable) {
        // Finish saving the rotation even if the screen that requested it is closed.
        val payload = JSONObject().put("refresh_token", current.refresh).put("client", "native")
        val response = transport.send("POST", "/api/auth/refresh", null, payload.toString())
        when (response.status) {
            200 -> Unit
            401 -> {
                clearSessionLocked()
                throw ApiFailure("unauthorized")
            }
            429 -> throw ApiFailure("rate_limited")
            else -> throw ApiFailure("internal_error")
        }
        val (_, refreshed) = decodeSession(response)
        if (refreshed.accountId != current.accountId) throw ApiFailure("invalid_response")
        saveSession(refreshed)
        refreshed
    }

    private suspend fun protected(method: String, path: String, body: JSONObject? = null): ApiResponse {
        val snapshot = sessionMutex.withLock { (session ?: throw ApiFailure("unauthorized")) to generation }
        val original = snapshot.first
        var response = transport.send(method, path, original.access, body?.toString())
        if (response.status != 401 || errorCode(response) != "unauthorized") return response
        val renewed = sessionMutex.withLock {
            val current = session ?: throw ApiFailure("unauthorized")
            if (generation != snapshot.second) throw ApiFailure("unauthorized")
            if (current !== original) current else refreshSession(current)
        }
        response = transport.send(method, path, renewed.access, body?.toString())
        if (response.status == 401 && errorCode(response) == "unauthorized") {
            sessionMutex.withLock { if (session === renewed) clearSessionLocked() }
            throw ApiFailure("unauthorized")
        }
        return response
    }

    internal suspend fun calendar(): CalendarData {
        val value = json(expect(protected("GET", "/api/calendar"), 200))
        return runCatching { parseCalendar(value) }.getOrElse { throw ApiFailure("invalid_response") }
    }
    internal suspend fun saveDiary(date: java.time.LocalDate, entry: DiaryDay) {
        expect(protected("PUT", "/api/diary/$date", entry.json()), 204)
    }
    internal suspend fun deleteDiary(date: java.time.LocalDate) {
        expect(protected("DELETE", "/api/diary/$date"), 204)
    }
    internal suspend fun savePeriod(id: String?, start: java.time.LocalDate, end: java.time.LocalDate?) {
        val body = JSONObject().put("start_date", start.toString()).put("end_date", end?.toString() ?: JSONObject.NULL)
        if (id == null) expect(protected("POST", "/api/cycle/periods", body), 201)
        else expect(protected("PUT", "/api/cycle/periods/${safeId(id)}", body), 204)
    }
    internal suspend fun deletePeriod(id: String) {
        expect(protected("DELETE", "/api/cycle/periods/${safeId(id)}"), 204)
    }
    internal suspend fun saveCycleSettings(length: Int?, duration: Int, method: String, context: String) {
        val body = JSONObject().put("cycle_length", length ?: JSONObject.NULL).put("period_length", duration)
            .put("length_method", method).put("context", context)
        expect(protected("PUT", "/api/cycle/settings", body), 204)
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

    suspend fun deleteSelf(password: String) {
        expect(protected("DELETE", "/api/me", JSONObject().put("current_password", password)), 204)
        clearSession()
    }
    suspend fun deleteUser(id: String) { expect(protected("DELETE", "/api/admin/users/${safeId(id)}"), 204) }
    suspend fun changeOwnPassword(currentPassword: String, newPassword: String) {
        expect(protected("POST", "/api/me/password", JSONObject().put("current_password", currentPassword).put("new_password", newPassword)), 204)
        clearSession()
    }
    suspend fun generateInvite(role: AccountRole? = null): GeneratedInvitation {
        val value = json(expect(protected("POST", if (role == null) "/api/me/invite" else "/api/admin/invites", if (role == null) JSONObject() else JSONObject().put("role", role.wire)), 201))
        val token = string(value, "token")
        if (InviteInput.parse(token) != token || !value.has("url")) throw ApiFailure("invalid_response")
        val url = if (value.isNull("url")) null else string(value, "url")
        if (url != null && InviteInput.parse(url) != token) throw ApiFailure("invalid_response")
        return GeneratedInvitation(token, url, string(value, "expires_at"), role?.label)
    }
    suspend fun revokeInvite(role: AccountRole? = null) { expect(protected("DELETE", if (role == null) "/api/me/invite" else "/api/admin/invites/${role.wire}"), 204) }
    suspend fun previewInvite(token: String): InvitePreview {
        val value = json(expect(transport.send("POST", "/api/invites/preview", null, JSONObject().put("token", token).toString()), 200))
        val kind = value.optString("kind", "pair")
        val role = when (value.optString("role", "partner")) { "tracker" -> AccountRole.TRACKER; "partner" -> AccountRole.PARTNER; else -> throw ApiFailure("invalid_response") }
        if (kind !in setOf("pair", "account")) throw ApiFailure("invalid_response")
        return InvitePreview(string(value, "tracker_display_name"), string(value, "expires_at"), kind, role)
    }
    suspend fun registerInvite(token: String, username: String, name: String, password: String): InviteRegistration {
        val body = JSONObject().put("token", token).put("username", username.trim().lowercase(Locale.ROOT))
            .put("display_name", name.trim()).put("password", password).put("client", "native")
        val value = json(expect(transport.send("POST", "/api/invites/register", null, body.toString()), 201))
        val user = account(value.getJSONObject("user"))
        val pair = if (value.isNull("pair")) null else pair(value.getJSONObject("pair"))
        if (user.role == AccountRole.ADMIN || pair != null && pair.partner.id != user.id) throw ApiFailure("invalid_response")
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
    private fun decodeSession(response: ApiResponse): Pair<Account, NativeSession> {
        val value = json(response)
        val user = account(value.getJSONObject("user"))
        if (!user.active) throw ApiFailure("invalid_response")
        val access = string(value, "access_token")
        val refresh = string(value, "refresh_token")
        if (access.length > 8192 || refresh.length > 4096 || access.any { it == '\r' || it == '\n' }) throw ApiFailure("invalid_response")
        return user to NativeSession(user.id, access, refresh)
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
    "session_storage" -> "Не удалось открыть защищённое хранилище сессии. Повторите попытку."
    "network_error" -> "Не удалось связаться с API. Проверьте подключение и адрес сервера."
    "invalid_response" -> "Сервер вернул некорректный ответ. Повторите попытку."
    else -> "Сервер временно недоступен. Повторите попытку."
}
