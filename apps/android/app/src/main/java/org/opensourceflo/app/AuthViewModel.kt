package org.opensourceflo.app

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

data class AuthState(
    val user: Account? = null,
    val ownPair: AccountPair? = null,
    val users: List<Account> = emptyList(),
    val pairs: List<AccountPair> = emptyList(),
    val busy: Boolean = true,
    val needsSetup: Boolean = false,
    val error: String? = null,
    val notice: String? = null,
    val createdUserVersion: Int = 0,
    val createdPairVersion: Int = 0
)

class AuthViewModel : ViewModel() {
    private val repository = AuthRepository()
    private val mutableState = MutableStateFlow(AuthState())
    val state = mutableState.asStateFlow()

    private var initialized = false

    init {
        runAction {
            mutableState.value = mutableState.value.copy(needsSetup = repository.needsSetup())
        }
    }

    fun login(username: String, password: String) = runAction {
        val user = repository.login(username, password)
        mutableState.value = AuthState(user = user, busy = true)
        loadDashboard()
    }

    fun reload() = runAction {
        if (mutableState.value.user == null) {
            mutableState.value = mutableState.value.copy(needsSetup = repository.needsSetup())
        } else loadDashboard()
    }

    fun logout() {
        if (mutableState.value.busy) return
        val needsSetup = mutableState.value.needsSetup
        mutableState.value = AuthState(busy = true, needsSetup = needsSetup)
        viewModelScope.launch {
            var error: String? = null
            try { repository.logout() }
            catch (cancelled: CancellationException) { throw cancelled }
            catch (_: Exception) { error = "Вы вышли на этом устройстве. Не удалось подтвердить завершение серверной сессии." }
            finally { mutableState.value = AuthState(busy = false, needsSetup = needsSetup, error = error, notice = if (error == null) "Вы вышли из аккаунта." else null) }
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

    private suspend fun loadDashboard() {
        val user = repository.me()
        if (user.role == AccountRole.ADMIN) {
            val users = repository.users()
            val pairs = repository.pairs()
            mutableState.value = mutableState.value.copy(user = user, users = users, pairs = pairs, ownPair = null)
        } else {
            val pair = repository.ownPair()
            mutableState.value = mutableState.value.copy(user = user, ownPair = pair, users = emptyList(), pairs = emptyList())
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
                    repository.clearSession()
                    mutableState.value = AuthState(busy = true, error = authErrorMessage(failure.code))
                } else mutableState.value = mutableState.value.copy(error = authErrorMessage(failure.code))
            } catch (_: Exception) {
                mutableState.value = mutableState.value.copy(error = authErrorMessage("invalid_response"))
            } finally { mutableState.value = mutableState.value.copy(busy = false) }
        }
    }
}
