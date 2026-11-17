package org.opensourceflo.app

import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test

class InvitationsTest {
    private val token = "a".repeat(64)
    private val tracker = """{"id":"10000000-0000-4000-8000-000000000001","username":"fixture_tracker","display_name":"Fixture Tracker","role":"tracker","is_active":true}"""
    private val partner = """{"id":"10000000-0000-4000-8000-000000000002","username":"fixture_partner","display_name":"Fixture Partner","role":"partner","is_active":true}"""
    private val pair = """{"id":"20000000-0000-4000-8000-000000000001","tracker":$tracker,"partner":$partner,"created_at":"2026-11-17T18:00:00Z"}"""
    private fun session(user: String) = """{"user":$user,"access_token":"fixture-access","refresh_token":"fixture-refresh","expires_in":900}"""
    private suspend fun failure(code: String, action: suspend () -> Unit) {
        try { action(); fail("Expected safe failure") } catch (error: ApiFailure) { assertEquals(code, error.code) }
    }

    @Test fun parsesCodeFragmentAndSeparateFrontendOriginWithoutFetchingIt() {
        assertEquals(token, InviteInput.parse(token))
        assertEquals(token, InviteInput.parse(token.uppercase()))
        assertEquals(token, InviteInput.parse("#invite=$token"))
        assertEquals(token, InviteInput.parse("https://frontend.example/#invite=$token"))
        assertEquals(token, InviteInput.parse("http://127.0.0.1:5173/#invite=$token"))
    }

    @Test fun rejectsUntrustedFormatsQueriesCredentialsAndLongInput() {
        listOf("javascript:alert(1)", "file:///#invite=$token", "https://u:p@frontend.example/#invite=$token",
            "https://frontend.example/?token=$token", "https://frontend.example/?x=1#invite=$token",
            "https://frontend.example/path#invite=$token", "#invite=$token&other=1", "a".repeat(63),
            "g".repeat(64), "https://frontend.example/#invite=%61${token.drop(1)}", "a".repeat(2049)
        ).forEach { assertNull(InviteInput.parse(it)) }
    }

    @Test fun previewUsesOnlyFixedApiRouteAndDoesNotAcceptOrRegister() = runBlocking {
        var calls = 0
        val repository = AuthRepository(ApiTransport { method, path, bearer, body ->
            calls++
            assertEquals("POST", method)
            assertEquals("/api/invites/preview", path)
            assertNull(bearer)
            assertEquals(token, JSONObject(body!!).getString("token"))
            assertFalse(body.contains("frontend.example"))
            ApiResponse(200, """{"tracker_display_name":"Fixture Tracker","expires_at":"2026-11-18T18:00:00Z"}""")
        })
        val parsed = InviteInput.parse("https://frontend.example/#invite=$token")!!
        assertEquals("Fixture Tracker", repository.previewInvite(parsed).trackerDisplayName)
        failure("unauthorized") { repository.me() }
        assertEquals(1, calls)
    }

    @Test fun wrongCurrentPassword401DoesNotRefreshOrLogout() = runBlocking {
        var refreshes = 0
        val repository = AuthRepository(ApiTransport { _, path, bearer, body ->
            when (path) {
                "/api/auth/login" -> ApiResponse(200, session(tracker))
                "/api/me/password" -> {
                    assertEquals("fixture-access", bearer)
                    assertEquals("fixture-current-password", JSONObject(body!!).getString("current_password"))
                    assertEquals("fixture-new-password", JSONObject(body).getString("new_password"))
                    ApiResponse(401, """{"error":"invalid_credentials"}""")
                }
                "/api/me" -> ApiResponse(200, tracker)
                else -> { refreshes++; error("Must not refresh on business credential error") }
            }
        })
        repository.login("fixture_tracker", "fixture-current-password")
        failure("invalid_credentials") { repository.changeOwnPassword("fixture-current-password", "fixture-new-password") }
        assertEquals("fixture_tracker", repository.me().username)
        assertEquals(0, refreshes)
    }

    @Test fun successfulPasswordChangeClearsSession() = runBlocking {
        val repository = AuthRepository(ApiTransport { method, path, _, _ ->
            when (path) {
                "/api/auth/login" -> ApiResponse(200, session(partner))
                "/api/me/password" -> { assertEquals("POST", method); ApiResponse(204, "") }
                else -> error("No further protected request should reach server")
            }
        })
        repository.login("fixture_partner", "fixture-current-password")
        repository.changeOwnPassword("fixture-current-password", "fixture-new-password")
        failure("unauthorized") { repository.me() }
    }

    @Test fun registrationIsExplicitNativePartnerWithoutImplicitLoginOrAccept() = runBlocking {
        var calls = 0
        val repository = AuthRepository(ApiTransport { method, path, bearer, body ->
            calls++
            assertEquals("POST", method)
            assertEquals("/api/invites/register", path)
            assertNull(bearer)
            val payload = JSONObject(body!!)
            assertEquals("native", payload.getString("client"))
            assertEquals(token, payload.getString("token"))
            assertEquals("fixture_partner", payload.getString("username"))
            assertFalse(payload.has("role"))
            ApiResponse(201, """{"user":$partner,"pair":$pair}""")
        })
        val created = repository.registerInvite(token, "FIXTURE_PARTNER", "Fixture Partner", "fixture-password-123")
        assertEquals(AccountRole.PARTNER, created.user.role)
        assertEquals(created.user.id, created.pair!!.partner.id)
        failure("unauthorized") { repository.me() }
        assertEquals(1, calls)
    }

    @Test fun acceptanceIsSeparateFromPreviewAndLogin() = runBlocking {
        var accepts = 0
        val repository = AuthRepository(ApiTransport { _, path, bearer, body ->
            when (path) {
                "/api/invites/preview" -> ApiResponse(200, """{"tracker_display_name":"Fixture Tracker","expires_at":"2026-11-18T18:00:00Z"}""")
                "/api/auth/login" -> ApiResponse(200, session(partner))
                "/api/me/invite/accept" -> {
                    accepts++
                    assertEquals("fixture-access", bearer)
                    assertEquals(token, JSONObject(body!!).getString("token"))
                    ApiResponse(201, pair)
                }
                else -> error("Unexpected route")
            }
        })
        repository.previewInvite(token)
        repository.login("fixture_partner", "fixture-password-123")
        assertEquals(0, accepts)
        assertEquals("fixture_tracker", repository.acceptInvite(token).tracker.username)
        assertEquals(1, accepts)
    }

    @Test fun ownerCanGenerateCodeWithoutPublicUrlAndRevokeIt() = runBlocking {
        var revoked = false
        val repository = AuthRepository(ApiTransport { method, path, bearer, _ ->
            when (path) {
                "/api/auth/login" -> ApiResponse(200, session(tracker))
                "/api/me/invite" -> {
                    assertEquals("fixture-access", bearer)
                    if (method == "DELETE") { revoked = true; ApiResponse(204, "") }
                    else ApiResponse(201, """{"token":"$token","url":null,"expires_at":"2026-11-18T18:00:00Z"}""")
                }
                else -> error("Unexpected route")
            }
        })
        repository.login("fixture_tracker", "fixture-password-123")
        assertEquals(token, repository.generateInvite().shareValue)
        repository.revokeInvite()
        assertTrue(revoked)
    }

    @Test fun unavailableInviteUsesSafeErrorCode() = runBlocking {
        val repository = AuthRepository(ApiTransport { _, _, _, _ -> ApiResponse(404, """{"error":"invite_unavailable"}""") })
        failure("invite_unavailable") { repository.previewInvite(token) }
    }

    @Test fun administrativeInvitationRegistersTrackerWithoutPair() = runBlocking {
        val repository = AuthRepository(ApiTransport { _, path, _, body ->
            when (path) {
                "/api/invites/preview" -> ApiResponse(200, """{"tracker_display_name":"Administrator","kind":"account","role":"tracker","expires_at":"2026-11-18T18:00:00Z"}""")
                "/api/invites/register" -> {
                    assertFalse(JSONObject(body!!).has("role"))
                    ApiResponse(201, """{"user":$tracker,"pair":null}""")
                }
                else -> error("Unexpected route")
            }
        })
        val preview = repository.previewInvite(token)
        assertEquals("account", preview.kind)
        assertEquals(AccountRole.TRACKER, preview.role)
        val created = repository.registerInvite(token, "fixture_tracker", "Fixture", "fixture-password-123")
        assertEquals(AccountRole.TRACKER, created.user.role)
        assertNull(created.pair)
    }
}
