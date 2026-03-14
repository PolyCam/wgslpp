package com.polycam.wgslpp

import com.intellij.openapi.fileTypes.LanguageFileType
import javax.swing.Icon

object WgslFileType : LanguageFileType(WgslLanguage) {
    override fun getName(): String = "WGSL"
    override fun getDescription(): String = "WGSL shader file"
    override fun getDefaultExtension(): String = "wgsl"
    override fun getIcon(): Icon? = null
}
