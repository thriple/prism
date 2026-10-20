plugins {
  alias(libs.plugins.android.application)
  alias(libs.plugins.kotlin.compose)
}

// Each GitHub build gets a higher version code so a new APK installs over the old one.
val buildNumber = (System.getenv("GITHUB_RUN_NUMBER") ?: "1").toInt()

android {
  namespace = "com.cougot.prism"
  compileSdk { version = release(36) { minorApiLevel = 1 } }

  defaultConfig {
    applicationId = "com.cougot.prism"
    minSdk = 26
    targetSdk = 36
    versionCode = buildNumber
    versionName = "1.0.$buildNumber"
  }

  signingConfigs {
    create("shared") {
      storeFile = file("${rootDir}/debug.keystore")
      storePassword = "android"
      keyAlias = "androiddebugkey"
      keyPassword = "android"
    }
  }

  buildTypes {
    debug { signingConfig = signingConfigs.getByName("shared") }
    release {
      isMinifyEnabled = false
      signingConfig = signingConfigs.getByName("shared")
    }
  }
  compileOptions {
    sourceCompatibility = JavaVersion.VERSION_11
    targetCompatibility = JavaVersion.VERSION_11
  }
  buildFeatures { compose = true }
}

dependencies {
  implementation(platform(libs.androidx.compose.bom))
  implementation(libs.androidx.activity.compose)
  implementation(libs.androidx.compose.foundation)
  implementation(libs.androidx.compose.material3)
  implementation(libs.androidx.compose.ui)
  implementation(libs.androidx.compose.ui.graphics)
  implementation(libs.androidx.core.ktx)
  implementation(libs.androidx.lifecycle.runtime.ktx)
  implementation(libs.kotlinx.coroutines.android)
  implementation(libs.media3.exoplayer)
  implementation(libs.media3.ui)
}
