import java.net.URI

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

val configuredApiUrl = providers.gradleProperty("API_BASE_URL").orNull?.trim()?.trimEnd('/')

fun validApiUrl(value: String, requireHttps: Boolean): Boolean = runCatching {
    val uri = URI(value)
    uri.host != null && uri.userInfo == null && uri.query == null && uri.fragment == null &&
        (uri.scheme == "https" || (!requireHttps && uri.scheme == "http"))
}.getOrDefault(false)

if (configuredApiUrl != null) {
    require(validApiUrl(configuredApiUrl, requireHttps = false)) {
        "API_BASE_URL must be an HTTP(S) base URL without credentials, query or fragment."
    }
}

android {
    namespace = "org.opensourceflo.app"
    compileSdk = 35

    defaultConfig {
        applicationId = "org.opensourceflo.app"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
    }

    buildTypes {
        debug {
            applicationIdSuffix = ".debug"
            versionNameSuffix = "-debug"
            buildConfigField("String", "API_BASE_URL", "\"${configuredApiUrl ?: "http://10.0.2.2:8080"}\"")
        }
        release {
            isMinifyEnabled = false
            buildConfigField("String", "API_BASE_URL", "\"${configuredApiUrl ?: ""}\"")
        }
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
}

val validateReleaseApiUrl by tasks.registering {
    doLast {
        require(configuredApiUrl != null && validApiUrl(configuredApiUrl, requireHttps = true)) {
            "Release requires an explicit HTTPS endpoint: -PAPI_BASE_URL=https://your-server.example"
        }
    }
}
tasks.matching { it.name == "preReleaseBuild" }.configureEach {
    dependsOn(validateReleaseApiUrl)
}

dependencies {
    implementation(platform("androidx.compose:compose-bom:2025.04.01"))
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.2")
}
