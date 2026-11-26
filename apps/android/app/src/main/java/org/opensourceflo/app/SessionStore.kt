package org.opensourceflo.app

import android.content.Context
import java.io.IOException
import java.security.GeneralSecurityException
import java.util.Base64
import javax.crypto.AEADBadTagException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONException
import org.json.JSONObject

internal interface SessionStore {
    suspend fun load(): NativeSession?
    suspend fun save(session: NativeSession)
    suspend fun clear()
}

internal class AndroidSessionStore(
    context: Context,
    baseUrl: String = BuildConfig.API_BASE_URL
) : SessionStore {
    private val preferences = context.getSharedPreferences("native_session", Context.MODE_PRIVATE)
    // Bind the saved credentials to the configured server, including scheme and port.
    private val cipher = PrivatePayloadCipher(KEY_ALIAS, baseUrl.trimEnd('/'))

    override suspend fun load(): NativeSession? = storage {
        val encoded = preferences.getString(SESSION, null) ?: return@storage null
        try {
            val text = cipher.decrypt(Base64.getDecoder().decode(encoded))
            if (text == null) {
                remove()
                return@storage null
            }
            val data = JSONObject(text)
            NativeSession(data.getString("accountId"), data.getString("access"), data.getString("refresh"))
        } catch (_: AEADBadTagException) {
            remove()
            null
        } catch (_: JSONException) {
            remove()
            null
        } catch (_: IllegalArgumentException) {
            remove()
            null
        }
    }

    override suspend fun save(session: NativeSession) = storage {
        val data = JSONObject().put("accountId", session.accountId)
            .put("access", session.access).put("refresh", session.refresh)
        val encrypted = cipher.encrypt(data.toString())
        val saved = preferences.edit().putString(SESSION, Base64.getEncoder().encodeToString(encrypted)).commit()
        if (!saved) throw ApiFailure("session_storage")
    }

    override suspend fun clear() = storage { remove() }

    private fun remove() {
        if (!preferences.edit().remove(SESSION).commit()) {
            // If the old ciphertext survives a disk error, it must not be usable after logout.
            cipher.deleteKey()
        }
    }

    private suspend fun <T> storage(action: () -> T): T = withContext(Dispatchers.IO) {
        try {
            action()
        } catch (_: GeneralSecurityException) {
            throw ApiFailure("session_storage")
        } catch (_: IOException) {
            throw ApiFailure("session_storage")
        }
    }

    private companion object {
        const val KEY_ALIAS = "flo.local.native-session"
        const val SESSION = "encrypted_session"
    }
}
