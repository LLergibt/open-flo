package org.opensourceflo.app

import android.content.Context
import android.util.AtomicFile
import java.io.File
import java.time.Duration
import java.time.Instant
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import org.json.JSONObject

internal data class CalendarSnapshot(val account: Account, val data: CalendarData, val savedAt: Instant)

internal class CalendarCache(context: Context) {
    private val file = AtomicFile(File(context.noBackupFilesDir, "calendar.snapshot"))
    private val cipher = PrivatePayloadCipher("flo.local.calendar", BuildConfig.API_BASE_URL.trimEnd('/'))
    private val mutex = Mutex()

    suspend fun remember(account: Account, payload: JSONObject) = access {
        val snapshot = JSONObject().put("id", account.id).put("username", account.username)
            .put("name", account.displayName).put("role", account.role.wire)
            .put("savedAt", Instant.now().toEpochMilli()).put("calendar", payload)
        val bytes = cipher.encrypt(snapshot.toString())
        val stream = file.startWrite()
        try {
            stream.write(bytes)
            file.finishWrite(stream)
        } catch (failure: Exception) {
            file.failWrite(stream)
            throw failure
        }
    }

    suspend fun recall(accountId: String): CalendarSnapshot? = access {
        val text = cipher.decrypt(file.readFully()) ?: return@access null
        val stored = JSONObject(text)
        val savedAt = Instant.ofEpochMilli(stored.getLong("savedAt"))
        val now = Instant.now()
        if (savedAt > now || Duration.between(savedAt, now) >= Duration.ofDays(7)) {
            file.delete()
            return@access null
        }
        if (stored.getString("id") != accountId) return@access null
        val role = AccountRole.parse(stored.getString("role"))
        if (role == AccountRole.ADMIN) return@access null
        val data = parseCalendar(stored.getJSONObject("calendar"))
        if (data.viewerId != accountId || data.owner != (role == AccountRole.TRACKER)) return@access null
        val account = Account(accountId, stored.getString("username"), stored.getString("name"), role, true)
        CalendarSnapshot(account, data, savedAt)
    }

    suspend fun forget() = access {
        file.delete()
        cipher.deleteKey()
    }

    private suspend fun <T> access(operation: () -> T): T? = mutex.withLock {
        withContext(Dispatchers.IO) {
            try {
                operation()
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (_: Exception) {
                // A missing, unreadable or unavailable copy must not block online use.
                null
            }
        }
    }
}
