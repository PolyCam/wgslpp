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

    // Marketplace builds bundle the LSP binary inside the extension under
    // `bin/`. The exact filename includes the platform suffix because vsce
    // packages a separate .vsix per target — see the release workflow.
    const exe = process.platform === 'win32' ? 'wgslpp-lsp.exe' : 'wgslpp-lsp';
    const candidates = [
        // Bundled with the published extension.
        path.join(__dirname, '..', 'bin', exe),
        // Local dev: `cargo build [--release] -p wgslpp-lsp` from the repo root.
        path.join(__dirname, '..', '..', 'target', 'release', exe),
        path.join(__dirname, '..', '..', 'target', 'debug', exe),
        // Last resort: PATH lookup (e.g. for users that built and installed
        // wgslpp-lsp manually).
        'wgslpp-lsp',
    ];

    for (const candidate of candidates) {
        // For absolute paths, only use them if the file exists; for the bare
        // command name we always fall through and let spawn handle the lookup.
        if (path.isAbsolute(candidate)) {
            try {
                require('fs').accessSync(candidate);
                return candidate;
            } catch {
                continue;
            }
        }
        return candidate;
    }
    return undefined;
}
