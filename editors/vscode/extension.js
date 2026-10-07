const vscode = require('vscode');
const fs = require('fs');
const path = require('path');

let client;
let watcher;
let pending = Promise.resolve();
let disposed = false;

function serverOptions(context) {
    const config = vscode.workspace.getConfiguration('tarn');
    const workspace = (vscode.workspace.workspaceFolders || [])[0]?.uri.fsPath;
    const checkout = path.resolve(context.extensionPath, '../..');
    const roots = [...new Set([workspace, checkout].filter(Boolean))];
    const candidates = roots.flatMap(root => ['debug', 'release'].map(mode => path.join(root, 'target', mode, 'tarn-lsp')));
    const configured = config.get('serverPath', '').trim();
    const command = configured
        ? (path.isAbsolute(configured) ? configured : path.resolve(workspace || process.cwd(), configured))
        : candidates.find(file => {
            try { fs.accessSync(file, fs.constants.X_OK); return fs.statSync(file).isFile(); }
            catch { return false; }
        }) || 'tarn-lsp';
    return { command, args: [], options: { cwd: workspace || process.cwd() } };
}
async function stop() {
    const running = client;
    client = undefined;
    try { if (running) await running.stop(); }
    finally { if (watcher) watcher.dispose(); watcher = undefined; }
}
async function start(context) {
    if (disposed || !vscode.workspace.getConfiguration('tarn').get('lsp.enabled', true)) return;
    // Syntax-only activation never loads a language client or starts a process.
    const { LanguageClient } = require('vscode-languageclient/node');
    watcher = vscode.workspace.createFileSystemWatcher('**/*.tarn');
    client = new LanguageClient('tarn', 'Tarn Language Server', serverOptions(context), {
        documentSelector: [{ scheme: 'file', language: 'tarn' }],
        synchronize: { fileEvents: watcher },
    });
    await client.start();
}
function enqueue(action) {
    pending = pending.then(action).catch(async error => {
        try { await stop(); } catch { /* Preserve the original startup error. */ }
        vscode.window.showErrorMessage(`Cannot start Tarn LSP. Build it with cargo build -p tarn-lsp or configure tarn.serverPath. ${error.message}`);
    });
    return pending;
}
async function activate(context) {
    disposed = false;
    context.subscriptions.push(vscode.commands.registerCommand('tarn.restartServer', () => enqueue(async () => {
        await stop();
        await start(context);
    })));
    context.subscriptions.push(vscode.workspace.onDidChangeConfiguration(event => {
        if (event.affectsConfiguration('tarn.lsp.enabled') || event.affectsConfiguration('tarn.serverPath')) {
            return enqueue(async () => { await stop(); await start(context); });
        }
    }));
    await enqueue(() => start(context));
}
async function deactivate() {
    disposed = true;
    await pending;
    await stop();
}
module.exports = { activate, deactivate };
