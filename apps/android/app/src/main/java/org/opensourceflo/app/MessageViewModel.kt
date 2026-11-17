package org.opensourceflo.app

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

sealed interface MessageState {
    data object Loading : MessageState
    data class Loaded(val value: DatabaseMessage) : MessageState
    data object Failed : MessageState
}

class MessageViewModel : ViewModel() {
    private val repository = MessageRepository()
    private val mutableState = MutableStateFlow<MessageState>(MessageState.Loading)
    val state = mutableState.asStateFlow()

    init {
        refresh()
    }

    fun refresh() {
        viewModelScope.launch {
            mutableState.value = MessageState.Loading
            try {
                mutableState.value = MessageState.Loaded(repository.load())
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (_: Exception) {
                mutableState.value = MessageState.Failed
            }
        }
    }
}
