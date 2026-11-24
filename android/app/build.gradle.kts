plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

val buildNumber = (System.getenv("GITHUB_RUN_NUMBER") ?: "1").toInt()

android {
    namespace = "com.squadtalk.app"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.squadtalk.app"
        minSdk = 29          // Android 10
        targetSdk = 35       // Android 15 (runs on 16 too)
        versionCode = buildNumber
        versionName = "0.3.$buildNumber"
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }
    }

    signingConfigs {
        create("release") {
            // GitHub uses the owner's private key when it's set up as a secret,
            // otherwise the public test key in this folder.
            storeFile = file(System.getenv("ST_KEYSTORE") ?: "test-key.jks")
            storePassword = System.getenv("ST_KEYSTORE_PASS") ?: "squadtalk-test"
            keyAlias = "squadtalk"
            keyPassword = System.getenv("ST_KEYSTORE_PASS") ?: "squadtalk-test"
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("release")
        }
    }

    externalNativeBuild {
        cmake {
            path = file("src/main/cpp/CMakeLists.txt")
            version = "3.22.1"
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    buildFeatures {
        compose = true
    }
    lint {
        abortOnError = false
        checkReleaseBuilds = false
    }
}

dependencies {
    val bom = platform("androidx.compose:compose-bom:2024.12.01")
    implementation(bom)
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
}
