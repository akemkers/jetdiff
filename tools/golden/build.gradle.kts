plugins {
    kotlin("jvm") version "2.4.0"
    application
}

repositories {
    mavenCentral()
    maven("https://www.jetbrains.com/intellij-repository/releases")
    // kotlinx-coroutines-intellij, transitively from util-base-multiplatform
    maven("https://cache-redirector.jetbrains.com/intellij-dependencies")
}

val intellijVersion = "262.10968.117"

dependencies {
    implementation("com.jetbrains.intellij.platform:util-base-multiplatform:$intellijVersion")
    implementation("com.jetbrains.intellij.platform:util-multiplatform:$intellijVersion")
    implementation("org.jetbrains:annotations:26.0.2")
}

sourceSets {
    main {
        kotlin.srcDirs("intellij-src/util-diff", "src/main/kotlin")
        java.srcDirs("intellij-src/diff-impl")
    }
}

kotlin {
    jvmToolchain(25)
    compilerOptions {
        // Corresponds to IntelliJ's -Xjvm-default=all
        jvmDefault.set(org.jetbrains.kotlin.gradle.dsl.JvmDefaultMode.NO_COMPATIBILITY)
        // Vendored code gives many deprecation warnings from multiplatform shims
        suppressWarnings.set(true)
    }
}

application {
    mainClass.set("jetdiff.golden.MainKt")
    // Turn on asserts in the IntelliJ code
    applicationDefaultJvmArgs = listOf("-ea")
}
