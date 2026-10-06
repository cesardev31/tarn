const vscode = require('vscode');
const fs = require('fs');
const path = require('path');
const { LanguageClient } = require('vscode-languageclient/node');
let client;
async function activate(context) {
    const config = vscode.workspace.getConfiguration('tarn');
    if (!config.get('lsp.enabled', true)) return;
    const configured = config.get('serverPath');
    const binary = process.platform === 'win32' ? 'tarn-lsp.exe' : 'tarn-lsp';
    const roots = [path.resolve(context.extensionPath, '../..'), ...(vscode.workspace.workspaceFolders || []).map(f => f.uri.fsPath)];
    const candidates = roots.flatMap(root => [path.join(root, 'target/debug', binary), path.join(root, 'target/release', binary)]);
    const command = configured || candidates.find(file => fs.existsSync(file)) || binary;
    const watcher = vscode.workspace.createFileSystemWatcher('**/*.tarn');
    client = new LanguageClient('tarn', 'Tarn Language Server', { command, args: [], options: { cwd: roots[0] } }, {
        documentSelector: [{ scheme: 'file', language: 'tarn' }],
        synchronize: { fileEvents: watcher },
    });
    context.subscriptions.push(watcher);
    context.subscriptions.push(vscode.workspace.onDidChangeConfiguration(async event => {
        if (event.affectsConfiguration('tarn.lsp.enabled') && !vscode.workspace.getConfiguration('tarn').get('lsp.enabled', true)) {
            await client.stop();
        }
    }));
    context.subscriptions.push(vscode.commands.registerCommand('tarn.restartServer', async () => {
        await client.stop();
        if (vscode.workspace.getConfiguration('tarn').get('lsp.enabled', true)) await client.start();
    }));
    try { await client.start(); }
    catch (error) {
        vscode.window.showErrorMessage(`No se pudo iniciar Tarn LSP. Ejecuta cargo build -p tarn-lsp o configura tarn.serverPath. ${error.message}`);
    }
}
async function deactivate() { if (client) await client.stop(); }
module.exports = { activate, deactivate };
