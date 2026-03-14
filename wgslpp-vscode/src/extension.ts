import * as path from 'path';
import * as vscode from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext) {
    const serverPath = getServerPath();

    if (!serverPath) {
        vscode.window.showWarningMessage(
            'wgslpp-lsp binary not found. Install it or set wgslpp.binary.path in settings.'
        );
        return;
    }

    const serverOptions: ServerOptions = {
        command: serverPath,
        args: [],
        transport: TransportKind.stdio,
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'wgsl' }],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.wgsl'),
        },
    };

    client = new LanguageClient(
        'wgslpp',
        'WGSL++ Language Server',
        serverOptions,
        clientOptions
    );

    // Register commands
    context.subscriptions.push(
        vscode.commands.registerCommand('wgslpp.validate', () => {
            vscode.window.showInformationMessage('Validation triggered by LSP on save/change.');
        }),
        vscode.commands.registerCommand('wgslpp.showPreprocessed', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) return;

            const doc = await vscode.workspace.openTextDocument({
                content: '// Preprocessed output would appear here.\n// This feature requires the wgslpp CLI.',
                language: 'wgsl',
            });
            await vscode.window.showTextDocument(doc, vscode.ViewColumn.Beside);
        }),
        vscode.commands.registerCommand('wgslpp.restartServer', async () => {
            if (client) {
                await client.restart();
                vscode.window.showInformationMessage('WGSL++ language server restarted.');
            }
        })
    );

    client.start();
    context.subscriptions.push({ dispose: () => client?.stop() });
}

export function deactivate(): Thenable<void> | undefined {
    return client?.stop();
}

function getServerPath(): string | undefined {
    const config = vscode.workspace.getConfiguration('wgslpp');
    const configPath = config.get<string>('binary.path');

    if (configPath && configPath.length > 0) {
        return configPath;
    }

    // Search common locations
    const candidates = [
        'wgslpp-lsp', // In PATH
        path.join(__dirname, '..', '..', 'target', 'release', 'wgslpp-lsp'),
        path.join(__dirname, '..', '..', 'target', 'debug', 'wgslpp-lsp'),
    ];

    // For now, return the first candidate and let the LSP client handle errors
    return candidates[0];
}
