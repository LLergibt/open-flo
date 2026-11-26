package org.opensourceflo.app

enum class AccountRole(val wire: String, val label: String) {
    ADMIN("admin", "Администратор"), TRACKER("tracker", "Владелец дневника"), PARTNER("partner", "Партнёр");
    companion object {
        fun parse(value: String): AccountRole = entries.firstOrNull { it.wire == value } ?: throw ApiFailure("invalid_response")
    }
}
data class Account(val id: String, val username: String, val displayName: String, val role: AccountRole, val active: Boolean)
data class AccountPair(val id: String, val tracker: Account, val partner: Account, val createdAt: String)
class ApiFailure(val code: String) : Exception(code)
internal class NativeSession(val accountId: String, val access: String, val refresh: String)
