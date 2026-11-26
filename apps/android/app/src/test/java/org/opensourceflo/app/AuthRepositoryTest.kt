package org.opensourceflo.app

import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.launch
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test

class AuthRepositoryTest {
    private val user = """{"id":"10000000-0000-4000-8000-000000000001","username":"fixture_admin","display_name":"Fixture Admin","role":"admin","is_active":true}"""
    private fun credentials(access: String = "fixture-access-a", refresh: String = "fixture-refresh-a") =
        """{"user":$user,"access_token":"$access","refresh_token":"$refresh","expires_in":900}"""
    private suspend fun failure(code: String, action: suspend () -> Unit) {
        try { action(); fail("Expected a safe error code") }
        catch (error: ApiFailure) { assertEquals(code, error.code) }
    }

    private class SavedSession : SessionStore {
        var credentials: NativeSession? = null

        override suspend fun load(): NativeSession? = credentials
        override suspend fun save(session: NativeSession) { credentials = session }
        override suspend fun clear() { credentials = null }
    }

    @Test fun parallel401UsesOneRotatingRefresh() = runBlocking {
        val oldCalls = AtomicInteger()
        val freshCalls = AtomicInteger()
        val refreshCalls = AtomicInteger()
        val bothStarted = CompletableDeferred<Unit>()
        val repository = AuthRepository(ApiTransport { _, path, bearer, body ->
            when (path) {
                "/api/auth/login" -> {
                    assertEquals("native", JSONObject(body!!).getString("client"))
                    ApiResponse(200, credentials())
                }
                "/api/auth/refresh" -> {
                    refreshCalls.incrementAndGet()
                    assertEquals("fixture-refresh-a", JSONObject(body!!).getString("refresh_token"))
                    assertEquals("native", JSONObject(body).getString("client"))
                    delay(25)
                    ApiResponse(200, credentials("fixture-access-b", "fixture-refresh-b"))
                }
                "/api/me" -> if (bearer == "fixture-access-a") {
                    if (oldCalls.incrementAndGet() == 2) bothStarted.complete(Unit)
                    bothStarted.await()
                    ApiResponse(401, """{"error":"unauthorized"}""")
                } else {
                    assertEquals("fixture-access-b", bearer)
                    freshCalls.incrementAndGet()
                    ApiResponse(200, user)
                }
                else -> error("Unexpected fixture route")
            }
        })
        repository.login("FIXTURE_ADMIN", "fixture-password-123")
        val users = listOf(async { repository.me() }, async { repository.me() }).awaitAll()
        assertEquals(2, users.size)
        assertEquals(2, oldCalls.get())
        assertEquals(2, freshCalls.get())
        assertEquals(1, refreshCalls.get())
    }

    @Test fun forbiddenDoesNotRefreshOrClearSession() = runBlocking {
        val refreshCalls = AtomicInteger()
        val repository = AuthRepository(ApiTransport { _, path, _, _ ->
            when (path) {
                "/api/auth/login" -> ApiResponse(200, credentials())
                "/api/admin/users" -> ApiResponse(403, """{"error":"forbidden"}""")
                "/api/me" -> ApiResponse(200, user)
                else -> { refreshCalls.incrementAndGet(); ApiResponse(500, "") }
            }
        })
        repository.login("fixture_admin", "fixture-password-123")
        failure("forbidden") { repository.users() }
        assertEquals("fixture_admin", repository.me().username)
        assertEquals(0, refreshCalls.get())
    }

    @Test fun failedRefreshClearsSessionAndNeverLoops() = runBlocking {
        val refreshCalls = AtomicInteger()
        val protectedCalls = AtomicInteger()
        val repository = AuthRepository(ApiTransport { _, path, _, _ ->
            when (path) {
                "/api/auth/login" -> ApiResponse(200, credentials())
                "/api/auth/refresh" -> { refreshCalls.incrementAndGet(); ApiResponse(401, """{"error":"unauthorized"}""") }
                else -> { protectedCalls.incrementAndGet(); ApiResponse(401, """{"error":"unauthorized"}""") }
            }
        })
        repository.login("fixture_admin", "fixture-password-123")
        failure("unauthorized") { repository.me() }
        failure("unauthorized") { repository.me() }
        assertEquals(1, refreshCalls.get())
        assertEquals(1, protectedCalls.get())
    }

    @Test fun retried401StopsAfterOneRefresh() = runBlocking {
        val refreshCalls = AtomicInteger()
        val protectedCalls = AtomicInteger()
        val repository = AuthRepository(ApiTransport { _, path, _, _ ->
            when (path) {
                "/api/auth/login" -> ApiResponse(200, credentials())
                "/api/auth/refresh" -> { refreshCalls.incrementAndGet(); ApiResponse(200, credentials("fixture-access-b", "fixture-refresh-b")) }
                else -> { protectedCalls.incrementAndGet(); ApiResponse(401, """{"error":"unauthorized"}""") }
            }
        })
        repository.login("fixture_admin", "fixture-password-123")
        failure("unauthorized") { repository.me() }
        failure("unauthorized") { repository.me() }
        assertEquals(1, refreshCalls.get())
        assertEquals(2, protectedCalls.get())
    }

    @Test fun logoutWithExpiredAccessUsesRefreshProofAndClearsMemory() = runBlocking {
        var loggedOut = false
        val repository = AuthRepository(ApiTransport { method, path, bearer, body ->
            when (path) {
                "/api/auth/login" -> ApiResponse(200, credentials("fixture-expired-access"))
                "/api/auth/logout" -> {
                    assertEquals("POST", method)
                    assertNull(bearer)
                    assertEquals("fixture-refresh-a", JSONObject(body!!).getString("refresh_token"))
                    assertEquals("native", JSONObject(body).getString("client"))
                    loggedOut = true
                    ApiResponse(204, "")
                }
                else -> error("Protected call must not run after logout")
            }
        })
        repository.login("fixture_admin", "fixture-password-123")
        repository.logout()
        assertTrue(loggedOut)
        failure("unauthorized") { repository.me() }
    }

    @Test fun failedServerLogoutStillClearsLocalSession() = runBlocking {
        val repository = AuthRepository(ApiTransport { _, path, _, _ ->
            if (path == "/api/auth/login") ApiResponse(200, credentials()) else throw ApiFailure("network_error")
        })
        repository.login("fixture_admin", "fixture-password-123")
        failure("network_error") { repository.logout() }
        failure("unauthorized") { repository.me() }
    }

    @Test fun restartReusesValidAccessAndLogoutClearsPersistedSessionEvenOffline() = runBlocking {
        val store = SavedSession()
        val transport = ApiTransport { _, path, bearer, _ ->
            when (path) {
                "/api/auth/login" -> ApiResponse(200, credentials())
                "/api/me" -> {
                    assertEquals("fixture-access-a", bearer)
                    ApiResponse(200, user)
                }
                "/api/auth/logout" -> throw ApiFailure("network_error")
                else -> error("Valid access should not rotate on restart")
            }
        }
        AuthRepository(transport, store).login("fixture_admin", "fixture-password-123")

        val reopened = AuthRepository(transport, store)
        assertTrue(reopened.restoreSession())
        assertEquals("fixture_admin", reopened.me().username)
        failure("network_error") { reopened.logout() }
        assertFalse(AuthRepository(transport, store).restoreSession())
    }

    @Test fun transientRefreshFailuresKeepTheSavedSessionUntilTheServerRevokesIt() = runBlocking {
        val store = SavedSession()
        val transport = ApiTransport { _, path, _, _ ->
            check(path == "/api/auth/login")
            ApiResponse(200, credentials())
        }
        AuthRepository(transport, store).login("fixture_admin", "fixture-password-123")

        val reopened = AuthRepository(ApiTransport { _, path, _, _ ->
            if (path == "/api/auth/refresh") throw ApiFailure("network_error")
            ApiResponse(401, """{"error":"unauthorized"}""")
        }, store)
        assertTrue(reopened.restoreSession())
        failure("network_error") { reopened.me() }

        val duringOutage = AuthRepository(ApiTransport { _, path, _, _ ->
            if (path == "/api/auth/refresh") ApiResponse(502, "")
            else ApiResponse(401, """{"error":"unauthorized"}""")
        }, store)
        assertTrue(duringOutage.restoreSession())
        failure("internal_error") { duringOutage.me() }

        val afterRevocation = AuthRepository(ApiTransport { _, _, _, _ ->
            ApiResponse(401, """{"error":"unauthorized"}""")
        }, store)
        assertTrue(afterRevocation.restoreSession())
        failure("unauthorized") { afterRevocation.me() }
        assertFalse(AuthRepository(transport, store).restoreSession())
    }

    @Test fun rotationIsSavedEvenWhenTheRequestingScreenIsClosed() = runBlocking {
        val store = SavedSession()
        val refreshing = CompletableDeferred<Unit>()
        val finishRefresh = CompletableDeferred<Unit>()
        val transport = ApiTransport { _, path, bearer, body ->
            when (path) {
                "/api/auth/login" -> ApiResponse(200, credentials())
                "/api/auth/refresh" -> {
                    assertEquals("fixture-refresh-a", JSONObject(body!!).getString("refresh_token"))
                    refreshing.complete(Unit)
                    finishRefresh.await()
                    ApiResponse(200, credentials("fixture-access-b", "fixture-refresh-b"))
                }
                "/api/me" -> if (bearer == "fixture-access-a") {
                    ApiResponse(401, """{"error":"unauthorized"}""")
                } else {
                    assertEquals("fixture-access-b", bearer)
                    ApiResponse(200, user)
                }
                else -> error("Unexpected fixture route")
            }
        }
        val repository = AuthRepository(transport, store)
        repository.login("fixture_admin", "fixture-password-123")
        val request = launch { repository.me() }
        refreshing.await()
        request.cancel()
        finishRefresh.complete(Unit)
        request.cancelAndJoin()

        assertEquals("fixture-refresh-b", store.credentials?.refresh)
        val reopened = AuthRepository(transport, store)
        assertTrue(reopened.restoreSession())
        assertEquals("fixture_admin", reopened.me().username)
    }

    @Test fun unsafeServerErrorTextNeverBecomesUiError() = runBlocking {
        val repository = AuthRepository(ApiTransport { _, path, _, _ ->
            if (path == "/api/auth/login") ApiResponse(200, credentials()) else ApiResponse(500, """{"error":"SQL secret fixture detail"}""")
        })
        repository.login("fixture_admin", "fixture-password-123")
        failure("internal_error") { repository.users() }
        assertFalse(authErrorMessage("SQL secret fixture detail").contains("SQL"))
    }
}
