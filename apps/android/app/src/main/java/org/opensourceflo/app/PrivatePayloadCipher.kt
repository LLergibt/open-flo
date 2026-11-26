package org.opensourceflo.app

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

internal class PrivatePayloadCipher(private val alias: String, scope: String) {
    private val scopeBytes = scope.toByteArray(Charsets.UTF_8)

    fun encrypt(text: String): ByteArray {
        val key = loadKey() ?: generateKey()
        val cipher = Cipher.getInstance(CIPHER)
        cipher.init(Cipher.ENCRYPT_MODE, key)
        cipher.updateAAD(scopeBytes)
        return cipher.iv + cipher.doFinal(text.toByteArray(Charsets.UTF_8))
    }

    fun decrypt(bytes: ByteArray): String? {
        if (bytes.size <= IV_BYTES) return null
        val key = loadKey() ?: return null
        val cipher = Cipher.getInstance(CIPHER)
        cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(128, bytes, 0, IV_BYTES))
        cipher.updateAAD(scopeBytes)
        return cipher.doFinal(bytes, IV_BYTES, bytes.size - IV_BYTES).toString(Charsets.UTF_8)
    }

    fun deleteKey() { keyStore().deleteEntry(alias) }

    private fun keyStore(): KeyStore = KeyStore.getInstance(PROVIDER).apply { load(null) }

    private fun loadKey(): SecretKey? = keyStore().getKey(alias, null) as? SecretKey

    private fun generateKey(): SecretKey = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, PROVIDER).run {
        val spec = KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .build()
        init(spec)
        generateKey()
    }

    private companion object {
        const val PROVIDER = "AndroidKeyStore"
        const val CIPHER = "AES/GCM/NoPadding"
        const val IV_BYTES = 12
    }
}
