package com.polycam.wgslpp

import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.LspServerSupportProvider
import com.intellij.platform.lsp.api.LspServerSupportProvider.LspServerStarter
import com.intellij.platform.lsp.api.ProjectWideLspServerDescriptor

class WgslppLspServerSupportProvider : LspServerSupportProvider {
    override fun fileOpened(
        project: Project,
        file: VirtualFile,
        serverStarter: LspServerStarter
    ) {
        if (file.extension == "wgsl") {
            serverStarter.ensureServerStarted(WgslppLspServerDescriptor(project))
        }
    }
}

class WgslppLspServerDescriptor(project: Project) :
    ProjectWideLspServerDescriptor(project, "WGSL++") {

    override fun isSupportedFile(file: VirtualFile): Boolean =
        file.extension == "wgsl"

    override fun createCommandLine(): GeneralCommandLine {
        val settings = WgslppSettings.getInstance()
        val binary = settings.binaryPath.ifBlank { "wgslpp-lsp" }
        return GeneralCommandLine(binary)
    }
}
