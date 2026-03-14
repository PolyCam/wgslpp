package com.polycam.wgslpp

import org.jetbrains.plugins.textmate.api.TextMateBundleProvider
import org.jetbrains.plugins.textmate.api.TextMateBundleProvider.PluginBundle
import java.nio.file.Path

class WgslTextMateBundleProvider : TextMateBundleProvider {
    override fun getBundles(): List<PluginBundle> {
        val url = this::class.java.classLoader.getResource("textmate")
            ?: return emptyList()
        return listOf(PluginBundle("WGSL", Path.of(url.toURI())))
    }
}
