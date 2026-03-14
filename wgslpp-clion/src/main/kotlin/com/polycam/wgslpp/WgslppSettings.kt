package com.polycam.wgslpp

import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.components.PersistentStateComponent
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.State
import com.intellij.openapi.components.Storage

@Service(Service.Level.APP)
@State(name = "WgslppSettings", storages = [Storage("wgslpp.xml")])
class WgslppSettings : PersistentStateComponent<WgslppSettings.State> {
    data class State(
        var binaryPath: String = "",
    )

    private var state = State()

    var binaryPath: String
        get() = state.binaryPath
        set(value) { state.binaryPath = value }

    override fun getState(): State = state
    override fun loadState(state: State) { this.state = state }

    companion object {
        fun getInstance(): WgslppSettings =
            ApplicationManager.getApplication().getService(WgslppSettings::class.java)
    }
}
