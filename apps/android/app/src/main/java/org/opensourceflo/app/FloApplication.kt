package org.opensourceflo.app

import android.app.Application

class FloApplication : Application() {
    // Every activity shares the same lock for refresh and credential storage.
    internal val authRepository by lazy {
        AuthRepository(HttpApiTransport(), AndroidSessionStore(this))
    }
}
