package org.opensourceflo.app

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

internal data class AuthState(
    val user: Account? = null,
    val calendar: CalendarData? = null,
    val calendarError: String? = null,
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
    private val repository = (application as FloApplication).authRepository
    private val mutableState = MutableStateFlow(AuthState(restoringSession = true))
    val state = mutableState.asStateFlow()

    private var initialized = false

    init {
        runAction { loadInitialState() }
    }

    fun login(username: String, password: String) = runAction {
        val user = repository.login(username, password)
        mutableState.value = AuthState(user = user, busy = true, pendingInvite = mutableState.value.pendingInvite)
        loadDashboard()
    }

    fun reload() = runAction {
        if (mutableState.value.user == null) loadInitialState() else loadDashboard()
    }

    private suspend fun loadInitialState() {
        if (repository.restoreSession()) {
            loadDashboard()
            mutableState.value = mutableState.value.copy(restoringSession = false)
        } else {
            mutableState.value = mutableState.value.copy(restoringSession = false)
            mutableState.value = mutableState.value.copy(needsSetup = repository.needsSetup())
        }
    }

    fun logout() {
        if (mutableState.value.busy) return
        val needsSetup = mutableState.value.needsSetup
        val pending = mutableState.value.pendingInvite
        mutableState.value = AuthState(busy = true, needsSetup = needsSetup, pendingInvite = pending)
        viewModelScope.launch {
            var error: String? = null
            try { repository.logout() }
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
        mutableState.value = AuthState(busy = true, notice = "Аккаунт удалён.")
    }
    fun changeOwnPassword(current: String, newPassword: String) = runAction {
        try { repository.changeOwnPassword(current, newPassword) }
        catch (failure: ApiFailure) {
            if (failure.code == "invalid_credentials") throw ApiFailure("current_password_incorrect")
            throw failure
        }
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

    fun refreshCalendar() = runAction { loadCalendar() }
    fun saveDiary(date: java.time.LocalDate, entry: DiaryDay) = mutateCalendar { repository.saveDiary(date, entry) }
    fun deleteDiary(date: java.time.LocalDate) = mutateCalendar { repository.deleteDiary(date) }
    fun savePeriod(id: String?, start: java.time.LocalDate, end: java.time.LocalDate?) =
        mutateCalendar { repository.savePeriod(id, start, end) }
    fun deletePeriod(id: String) = mutateCalendar { repository.deletePeriod(id) }
    fun saveCycleSettings(length: Int?, duration: Int, method: String, context: String) =
        mutateCalendar { repository.saveCycleSettings(length, duration, method, context) }

    private fun mutateCalendar(action: suspend () -> Unit) = runAction {
        if (mutableState.value.user?.role != AccountRole.TRACKER || mutableState.value.calendar?.owner != true) {
            throw ApiFailure("forbidden")
        }
        action()
        loadCalendar()
        mutableState.value = mutableState.value.copy(notice = "Изменения сохранены.")
    }
    private suspend fun loadCalendar() {
        val role = mutableState.value.user?.role ?: return
        if (role == AccountRole.ADMIN) return
        if (role == AccountRole.PARTNER) mutableState.value = mutableState.value.copy(calendar = null)
        try {
            val next = repository.calendar()
            if (next.owner != (role == AccountRole.TRACKER)) throw ApiFailure("invalid_response")
            mutableState.value = mutableState.value.copy(calendar = next, calendarError = null)
        } catch (failure: ApiFailure) {
            if (failure.code == "unauthorized") throw failure
            val message = if (failure.code == "forbidden" && role == AccountRole.PARTNER) {
                "Календарь появится после привязки к владельцу."
            } else authErrorMessage(failure.code)
            mutableState.value = mutableState.value.copy(calendar = null, calendarError = message)
        }
    }

    private suspend fun loadDashboard() {
        val user = repository.me()
        if (user.role == AccountRole.ADMIN) {
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
                    mutableState.value = AuthState(busy = true, error = authErrorMessage(failure.code), pendingInvite = mutableState.value.pendingInvite)
                } else mutableState.value = mutableState.value.copy(error = authErrorMessage(failure.code), pendingInvite = if (failure.code == "invite_unavailable") null else mutableState.value.pendingInvite)
            } catch (_: Exception) {
                mutableState.value = mutableState.value.copy(error = authErrorMessage("invalid_response"))
            } finally { mutableState.value = mutableState.value.copy(busy = false) }
        }
    }
}
