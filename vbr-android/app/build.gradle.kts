plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "dev.vbr.android"
    compileSdk = 36

    defaultConfig {
        applicationId = "dev.vbr.android"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
}

// Copy a curated set of core-language examples from the repo so they never drift.
val exampleFiles = listOf(
    "hello.vbr",
    "maths.vbr",
    "logic.vbr",
    "constants.vbr",
    "conversions.vbr",
    "string_funcs.vbr",
    "match.vbr",
    "match_guards.vbr",
    "doloop.vbr",
    "iterators.vbr",
    "arrays.vbr",
    "vec.vbr",
    "hashmap.vbr",
    "list_literal.vbr",
    "functions.vbr",
    "methods.vbr",
    "enums.vbr",
    "enum_payloads.vbr",
    "sum_types.vbr",
    "result.vbr",
    "option.vbr",
    "compound_assign.vbr",
    "byref.vbr",
    "memory.vbr",
)

val copyExamples = tasks.register<Copy>("copyExamples") {
    val repoExamples = rootProject.projectDir.resolve("../examples")
    from(repoExamples) {
        include(exampleFiles)
    }
    into(layout.projectDirectory.dir("src/main/assets/examples"))
}

tasks.named("preBuild") {
    dependsOn(copyExamples)
}
