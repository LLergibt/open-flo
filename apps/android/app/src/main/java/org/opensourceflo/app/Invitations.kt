package org.opensourceflo.app

import java.net.URI
import java.util.Locale

data class InvitePreview(val trackerDisplayName: String, val expiresAt: String)
class PendingInvitation internal constructor(internal val token: String, val preview: InvitePreview)
class GeneratedInvitation(val token: String, val url: String?, val expiresAt: String) {
    val shareValue: String get() = url ?: token
}
data class InviteRegistration(val user: Account, val pair: AccountPair)

internal object InviteInput {
    private val tokenPattern = Regex("[a-fA-F0-9]{64}")
    private val fragmentPattern = Regex("invite=([a-fA-F0-9]{64})")
    fun parse(input: String): String? {
        val value = input.trim()
        if (value.length > 2048 || value.any { it == '\r' || it == '\n' }) return null
        if (tokenPattern.matches(value)) return value.lowercase(Locale.ROOT)
        val fragment = if (value.startsWith("#")) value.drop(1) else {
            val uri = runCatching { URI(value) }.getOrNull() ?: return null
            if (uri.scheme !in setOf("https", "http") || uri.host == null || uri.rawUserInfo != null ||
                uri.rawQuery != null || uri.rawPath !in setOf("", "/")) return null
            uri.rawFragment ?: return null
        }
        return fragmentPattern.matchEntire(fragment)?.groupValues?.get(1)?.lowercase(Locale.ROOT)
    }
}
