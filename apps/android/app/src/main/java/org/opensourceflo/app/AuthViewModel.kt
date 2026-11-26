package org.opensourceflo.app

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import java.time.Instant
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.withContext
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

internal data class AuthState(
    val user: Account? = null,
    val calendar: CalendarData? = null,
    val calendarError: String? = null,
    val calendarSavedAt: Instant? = null,
    val ownPair: AccountPair? = null,
    val users: List<Account> = emptyList(),
    val pairs: List<AccountPair> = emptyList(),
    val busy: Boolean = true,
    val needsSetup: Boolean = false,
    val restoringSession: Boolean = false,
    val error: String? = null,
    val notice: String? = null,
    val createdUserVersion: Int = 0,
    val createdPairVersion: Int = 0,
    val pendingInvite: PendingInvitation? = null,
    val generatedInvite: GeneratedInvitation? = null
)

internal class AuthViewModel(application: Application) : AndroidViewModel(application) {
    private val app = application as FloApplication
    private val repository = app.authRepository
    private val cache = app.calendarCache
    private val mutableState = MutableStateFlow(AuthState(restoringSession = true))
    val state = mutableState.asStateFlow()

    private var initialized = false

    init {
        runAction { loadInitialState() }
    }

    fun login(username: String, password: String) = runAction {
        cache.forget()
        val user = repository.login(username, password)
        mutableState.value = AuthState(user = user, busy = true, pendingInvite = mutableState.value.pendingInvite)
        loadDashboard()
    }

    fun reload() = runAction {
        if (mutableState.value.user == null || mutableState.value.calendarSavedAt != null) loadInitialState() else loadDashboard()
    }

    private suspend fun loadInitialState() {
        if (repository.restoreSession()) {
            try {
                loadDashboard()
                mutableState.value = mutableState.value.copy(restoringSession = false)
            } catch (failure: ApiFailure) {
                if (!failure.transient() || !showCachedCalendar()) {
                    if (mutableState.value.calendarSavedAt != null) {
                        mutableState.value = AuthState(restoringSession = true)
                    }
                    throw failure
                }
            }
        } else {
            cache.forget()
            mutableState.value = mutableState.value.copy(restoringSession = false)
            mutableState.value = mutableState.value.copy(needsSetup = repository.needsSetup())
        }
    }

    private fun ApiFailure.transient() = code == "network_error" || code == "internal_error"

    private suspend fun showCachedCalendar(): Boolean {
        val accountId = repository.sessionAccountId() ?: return false
        val snapshot = cache.recall(accountId) ?: return false
        mutableState.value = AuthState(
            user = snapshot.account, calendar = snapshot.data, calendarSavedAt = snapshot.savedAt, busy = true
        )
        return true
    }

    fun logout() {
        if (mutableState.value.busy) return
        val needsSetup = mutableState.value.needsSetup
        val pending = mutableState.value.pendingInvite
        mutableState.value = AuthState(busy = true, needsSetup = needsSetup, pendingInvite = pending)
        viewModelScope.launch {
            var error: String? = null
            try {
                withContext(NonCancellable) { cache.forget() }
                repository.logout()
            }
            catch (cancelled: CancellationException) { throw cancelled }
            catch (failure: ApiFailure) {
                error = if (failure.code == "session_storage") authErrorMessage(failure.code)
                else "Вы вышли на этом устройстве. Не удалось подтвердить завершение серверной сессии."
            }
            finally { mutableState.value = AuthState(busy = false, needsSetup = needsSetup, error = error, notice = if (error == null) "Вы вышли из аккаунта." else null, pendingInvite = pending) }
        }
    }

    fun createUser(username: String, name: String, password: String, role: AccountRole) = adminAction {
        repository.createUser(username, name, password, role)
        mutableState.value = mutableState.value.copy(createdUserVersion = mutableState.value.createdUserVersion + 1)
        loadDashboard()
        mutableState.value = mutableState.value.copy(notice = "Аккаунт создан.")
    }
    fun setActive(user: Account, active: Boolean) = adminAction {
        if (user.id == mutableState.value.user?.id && !active) throw ApiFailure("invalid_input")
        repository.setActive(user.id, active)
        loadDashboard()
        mutableState.value = mutableState.value.copy(notice = if (active) "Аккаунт включён." else "Аккаунт отключён. Его сессии завершены.")
    }
    fun resetPassword(user: Account, password: String) = adminAction {
        repository.resetPassword(user.id, password)
        if (user.id == mutableState.value.user?.id) {
            repository.clearSession()
            cache.forget()
            mutableState.value = AuthState(busy = true, notice = "Пароль изменён. Войдите снова.")
        } else {
            loadDashboard()
            mutableState.value = mutableState.value.copy(notice = "Пароль изменён. Сессии аккаунта завершены.")
        }
    }
    fun createPair(trackerId: String, partnerId: String) = adminAction {
        repository.createPair(trackerId, partnerId)
        mutableState.value = mutableState.value.copy(createdPairVersion = mutableState.value.createdPairVersion + 1)
        loadDashboard()
        mutableState.value = mutableState.value.copy(notice = "Аккаунты связаны.")
    }
    fun unlinkPair(pair: AccountPair) = adminAction {
        repository.unlinkPair(pair.id)
        loadDashboard()
        mutableState.value = mutableState.value.copy(notice = "Связь прекращена.")
    }
    fun unlinkOwnPair() = runAction {
        repository.unlinkOwnPair()
        cache.forget()
        loadDashboard()
        mutableState.value = mutableState.value.copy(notice = "Связь прекращена.")
    }

    fun deleteUser(user: Account) = adminAction {
        repository.deleteUser(user.id)
        loadDashboard()
        mutableState.value = mutableState.value.copy(notice = "Аккаунт удалён.")
    }
    fun deleteSelf(password: String) = runAction {
        try { repository.deleteSelf(password) }
        catch (failure: ApiFailure) {
            if (failure.code == "invalid_credentials") throw ApiFailure("current_password_incorrect")
            throw failure
        }
        cache.forget()
        mutableState.value = AuthState(busy = true, notice = "Аккаунт удалён.")
    }
    fun changeOwnPassword(current: String, newPassword: String) = runAction {
        try { repository.changeOwnPassword(current, newPassword) }
        catch (failure: ApiFailure) {
            if (failure.code == "invalid_credentials") throw ApiFailure("current_password_incorrect")
            throw failure
        }
        cache.forget()
        mutableState.value = AuthState(busy = true, notice = "Пароль изменён. Войдите снова.", pendingInvite = mutableState.value.pendingInvite)
    }
    fun generateInvite(role: AccountRole? = null) = runAction {
        if (role == null && (mutableState.value.user?.role != AccountRole.TRACKER || mutableState.value.ownPair != null)) throw ApiFailure("forbidden")
        mutableState.value = mutableState.value.copy(generatedInvite = repository.generateInvite(role), notice = "Приглашение создано. Передайте его только выбранному партнёру.")
    }
    fun revokeInvite(role: AccountRole? = null) = runAction {
        repository.revokeInvite(role)
        mutableState.value = mutableState.value.copy(generatedInvite = null, notice = "Приглашения отозваны.")
    }
    fun previewInvite(input: String) = runAction {
        mutableState.value = mutableState.value.copy(pendingInvite = null)
        val token = InviteInput.parse(input) ?: throw ApiFailure("invalid_invite")
        val preview = repository.previewInvite(token)
        mutableState.value = mutableState.value.copy(pendingInvite = PendingInvitation(token, preview))
    }
    fun cancelInvite() {
        if (!mutableState.value.busy) mutableState.value = mutableState.value.copy(pendingInvite = null, error = null, notice = "Приглашение отменено.")
    }
    fun registerInvitedPartner(username: String, name: String, password: String) = runAction {
        if (mutableState.value.user != null) throw ApiFailure("forbidden")
        val invitation = mutableState.value.pendingInvite ?: throw ApiFailure("invite_unavailable")
        repository.registerInvite(invitation.token, username, name, password)
        mutableState.value = AuthState(busy = true, notice = "Аккаунт создан. Если вход не завершится, войдите с новым паролем.")
        cache.forget()
        val user = repository.login(username, password)
        mutableState.value = AuthState(user = user, busy = true, notice = "Аккаунт создан. Приглашение принято.")
        loadDashboard()
    }
    fun acceptInvite() = runAction {
        if (mutableState.value.user?.role != AccountRole.PARTNER || mutableState.value.ownPair != null) throw ApiFailure("forbidden")
        val invitation = mutableState.value.pendingInvite ?: throw ApiFailure("invite_unavailable")
        val pair = repository.acceptInvite(invitation.token)
        mutableState.value = mutableState.value.copy(ownPair = pair, pendingInvite = null, notice = "Приглашение принято. Аккаунты связаны.")
        loadDashboard()
    }

    fun refreshCalendar() = runAction {
        if (mutableState.value.calendarSavedAt != null) loadInitialState() else loadCalendar()
    }
    fun savePartnerDiary(pairId: String, date: java.time.LocalDate, entry: DiaryDay) =
        mutateCalendar(AccountRole.PARTNER) { repository.savePartnerDiary(pairId, date, entry) }
    fun deletePartnerDiary(pairId: String, date: java.time.LocalDate) =
        mutateCalendar(AccountRole.PARTNER) { repository.deletePartnerDiary(pairId, date) }
    fun saveDiary(date: java.time.LocalDate, entry: DiaryDay) = mutateCalendar { repository.saveDiary(date, entry) }
    fun deleteDiary(date: java.time.LocalDate) = mutateCalendar { repository.deleteDiary(date) }
    fun savePeriod(id: String?, start: java.time.LocalDate, end: java.time.LocalDate?) =
        mutateCalendar { repository.savePeriod(id, start, end) }
    fun deletePeriod(id: String) = mutateCalendar { repository.deletePeriod(id) }
    fun saveCycleSettings(length: Int?, duration: Int, method: String, context: String) =
        mutateCalendar { repository.saveCycleSettings(length, duration, method, context) }

    private fun mutateCalendar(role: AccountRole = AccountRole.TRACKER, action: suspend () -> Unit) = runAction {
        val current = mutableState.value
        val calendar = current.calendar ?: throw ApiFailure("forbidden")
        if (current.calendarSavedAt != null || current.user?.role != role ||
            calendar.owner != (role == AccountRole.TRACKER) ||
            role == AccountRole.PARTNER && calendar.partnerPairId == null
        ) throw ApiFailure("forbidden")
        try {
            action()
        } catch (failure: ApiFailure) {
            if (failure.transient()) showCachedCalendar()
            if (failure.code == "forbidden") {
                cache.forget()
                mutableState.value = mutableState.value.copy(calendar = null, calendarSavedAt = null)
            }
            throw failure
        }
        loadCalendar()
        mutableState.value = mutableState.value.copy(notice = "Изменения сохранены.")
    }

    private suspend fun loadCalendar() {
        val account = mutableState.value.user ?: return
        if (account.role == AccountRole.ADMIN) return
        try {
            val payload = repository.calendar()
            val next = try { parseCalendar(payload) } catch (_: Exception) { throw ApiFailure("invalid_response") }
            if (next.owner != (account.role == AccountRole.TRACKER) ||
                next.viewerId != null && next.viewerId != account.id
            ) throw ApiFailure("forbidden")
            if (next.viewerId == account.id) cache.remember(account, payload)
            mutableState.value = mutableState.value.copy(calendar = next, calendarError = null, calendarSavedAt = null)
        } catch (failure: ApiFailure) {
            if (failure.code == "unauthorized") throw failure
            if (failure.transient() && showCachedCalendar()) return
            if (failure.code == "forbidden") cache.forget()
            val message = if (failure.code == "forbidden" && account.role == AccountRole.PARTNER) {
                "Календарь появится после привязки к владельцу."
            } else authErrorMessage(failure.code)
            mutableState.value = mutableState.value.copy(calendar = null, calendarError = message, calendarSavedAt = null)
        }
    }

    private suspend fun loadDashboard() {
        val user = repository.me()
        if (user.role == AccountRole.ADMIN) {
            cache.forget()
            val users = repository.users()
            val pairs = repository.pairs()
            mutableState.value = mutableState.value.copy(user = user, users = users, pairs = pairs, ownPair = null)
        } else {
            val pair = repository.ownPair()
            mutableState.value = mutableState.value.copy(user = user, ownPair = pair, users = emptyList(), pairs = emptyList(), generatedInvite = if (pair != null) null else mutableState.value.generatedInvite)
            loadCalendar()
        }
    }
    private fun adminAction(action: suspend () -> Unit) = runAction {
        if (mutableState.value.user?.role != AccountRole.ADMIN) throw ApiFailure("forbidden")
        action()
    }
    private fun runAction(action: suspend () -> Unit) {
        if (mutableState.value.busy && initialized) return
        initialized = true
        mutableState.value = mutableState.value.copy(busy = true, error = null, notice = null)
        viewModelScope.launch {
            try { action() }
            catch (cancelled: CancellationException) { throw cancelled }
            catch (failure: ApiFailure) {
                if (failure.code == "unauthorized") {
                    cache.forget()
                    mutableState.value = AuthState(busy = true, error = authErrorMessage(failure.code), pendingInvite = mutableState.value.pendingInvite)
                } else mutableState.value = mutableState.value.copy(error = authErrorMessage(failure.code), pendingInvite = if (failure.code == "invite_unavailable") null else mutableState.value.pendingInvite)
            } catch (_: Exception) {
                mutableState.value = mutableState.value.copy(error = authErrorMessage("invalid_response"))
            } finally { mutableState.value = mutableState.value.copy(busy = false) }
        }
    }
}
