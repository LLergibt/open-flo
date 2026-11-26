package org.opensourceflo.app

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.io.IOException
import java.security.GeneralSecurityException
import java.security.KeyStore
import java.util.Base64
import javax.crypto.AEADBadTagException
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
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
    private val origin = baseUrl.trimEnd('/').toByteArray(Charsets.UTF_8)

    override suspend fun load(): NativeSession? = storage {
        val encoded = preferences.getString(SESSION, null) ?: return@storage null
        try {
            val bytes = Base64.getDecoder().decode(encoded)
            val key = keyStore().getKey(KEY_ALIAS, null) as? SecretKey
            if (key == null || bytes.size <= IV_BYTES) {
                remove()
                return@storage null
            }
            val cipher = Cipher.getInstance(CIPHER)
            cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(128, bytes, 0, IV_BYTES))
            cipher.updateAAD(origin)
            val data = JSONObject(cipher.doFinal(bytes, IV_BYTES, bytes.size - IV_BYTES).toString(Charsets.UTF_8))
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
        val key = (keyStore().getKey(KEY_ALIAS, null) as? SecretKey) ?: generateKey()
        val cipher = Cipher.getInstance(CIPHER)
        cipher.init(Cipher.ENCRYPT_MODE, key)
        cipher.updateAAD(origin)
        val data = JSONObject().put("accountId", session.accountId)
            .put("access", session.access).put("refresh", session.refresh)
        val encrypted = cipher.iv + cipher.doFinal(data.toString().toByteArray(Charsets.UTF_8))
        val saved = preferences.edit().putString(SESSION, Base64.getEncoder().encodeToString(encrypted)).commit()
        if (!saved) throw ApiFailure("session_storage")
    }

    override suspend fun clear() = storage { remove() }

    private fun remove() {
        if (!preferences.edit().remove(SESSION).commit()) {
            // If the old ciphertext survives a disk error, it must not be usable after logout.
            keyStore().deleteEntry(KEY_ALIAS)
        }
    }

    private fun keyStore(): KeyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }

    private fun generateKey(): SecretKey = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").run {
        val spec = KeyGenParameterSpec.Builder(KEY_ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .build()
        init(spec)
        generateKey()
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
        const val CIPHER = "AES/GCM/NoPadding"
        const val IV_BYTES = 12
    }
}
