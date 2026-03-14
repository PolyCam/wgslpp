plugins {
    id("java")
    id("org.jetbrains.kotlin.jvm") version "2.1.0"
    id("org.jetbrains.intellij.platform") version "2.2.1"
}

group = "com.polycam"
version = "0.1.0"

repositories {
    mavenCentral()
    intellijPlatform {
        defaultRepositories()
    }
}

dependencies {
    intellijPlatform {
        clion("2025.1") // baseline; works with 2025.1+
        bundledPlugin("org.jetbrains.plugins.textmate")
    }
}

kotlin {
    jvmToolchain(21)
}

intellijPlatform {
    pluginConfiguration {
        id = "com.polycam.wgslpp"
        name = "WGSL++"
        version = project.version.toString()
        description = "WGSL language support with preprocessor, validation, and optimization via wgslpp-lsp."
        vendor {
            name = "Polycam"
        }
        ideaVersion {
            sinceBuild = "251"   // 2025.1+
        }
    }
}

tasks {
    buildSearchableOptions {
        enabled = false // speeds up build for local-only plugin
    }
}
