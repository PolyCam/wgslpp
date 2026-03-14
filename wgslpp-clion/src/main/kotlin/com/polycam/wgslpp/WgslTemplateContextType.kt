package com.polycam.wgslpp

import com.intellij.codeInsight.template.TemplateActionContext
import com.intellij.codeInsight.template.TemplateContextType

class WgslTemplateContextType : TemplateContextType("WGSL") {
    override fun isInContext(context: TemplateActionContext): Boolean =
        context.file.name.endsWith(".wgsl")
}
