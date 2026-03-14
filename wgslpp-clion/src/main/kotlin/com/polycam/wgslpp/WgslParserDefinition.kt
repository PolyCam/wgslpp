package com.polycam.wgslpp

import com.intellij.extapi.psi.PsiFileBase
import com.intellij.lang.ASTNode
import com.intellij.lang.ParserDefinition
import com.intellij.lang.PsiParser
import com.intellij.lexer.EmptyLexer
import com.intellij.lexer.Lexer
import com.intellij.openapi.project.Project
import com.intellij.psi.FileViewProvider
import com.intellij.psi.PsiElement
import com.intellij.psi.PsiFile
import com.intellij.psi.tree.IFileElementType
import com.intellij.psi.tree.TokenSet

/**
 * Minimal parser definition. Actual parsing, diagnostics, and navigation
 * are provided by the LSP server. The TextMate grammar handles highlighting.
 */
class WgslParserDefinition : ParserDefinition {
    companion object {
        val FILE = IFileElementType(WgslLanguage)
    }

    override fun createLexer(project: Project?): Lexer = EmptyLexer()
    override fun createParser(project: Project?): PsiParser = PsiParser { _, builder ->
        val marker = builder.mark()
        while (!builder.eof()) builder.advanceLexer()
        marker.done(FILE)
        builder.treeBuilt
    }

    override fun getFileNodeType(): IFileElementType = FILE
    override fun getCommentTokens(): TokenSet = TokenSet.EMPTY
    override fun getStringLiteralElements(): TokenSet = TokenSet.EMPTY
    override fun createElement(node: ASTNode?): PsiElement =
        throw UnsupportedOperationException("Not used")

    override fun createFile(viewProvider: FileViewProvider): PsiFile =
        object : PsiFileBase(viewProvider, WgslLanguage) {
            override fun getFileType() = WgslFileType
        }
}
