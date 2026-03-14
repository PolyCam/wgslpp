package com.polycam.wgslpp

import com.intellij.openapi.fileChooser.FileChooserDescriptorFactory
import com.intellij.openapi.options.Configurable
import com.intellij.openapi.ui.TextFieldWithBrowseButton
import com.intellij.util.ui.FormBuilder
import javax.swing.JComponent
import javax.swing.JPanel

class WgslppSettingsConfigurable : Configurable {
    private var panel: JPanel? = null
    private var binaryPathField: TextFieldWithBrowseButton? = null

    override fun getDisplayName(): String = "WGSL++"

    override fun createComponent(): JComponent {
        val field = TextFieldWithBrowseButton()
        @Suppress("DEPRECATION")
        field.addBrowseFolderListener(
            "Select wgslpp-lsp Binary",
            "Path to the wgslpp-lsp executable",
            null,
            FileChooserDescriptorFactory.createSingleFileDescriptor()
        )
        binaryPathField = field
        panel = FormBuilder.createFormBuilder()
            .addLabeledComponent("wgslpp-lsp binary path:", field)
            .addComponentFillVertically(JPanel(), 0)
            .panel
        return panel!!
    }

    override fun isModified(): Boolean {
        val settings = WgslppSettings.getInstance()
        return binaryPathField?.text != settings.binaryPath
    }

    override fun apply() {
        val settings = WgslppSettings.getInstance()
        settings.binaryPath = binaryPathField?.text ?: ""
    }

    override fun reset() {
        val settings = WgslppSettings.getInstance()
        binaryPathField?.text = settings.binaryPath
    }

    override fun disposeUIResources() {
        panel = null
        binaryPathField = null
    }
}
