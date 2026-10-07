// Real VS Code extension-host acceptance. Run using --extensionTestsPath.
const vscode = require('vscode');
const assert = require('assert/strict');
const fs = require('fs');
const path = require('path');

async function until(predicate, description) {
    const deadline = Date.now() + 15000;
    while (Date.now() < deadline) {
        const value = await predicate();
        if (value) return value;
        await new Promise(resolve => setTimeout(resolve, 100));
    }
    throw new Error(`Timed out: ${description}`);
}
async function run() {
    const workspace = vscode.workspace.workspaceFolders[0].uri;
    const file = vscode.Uri.joinPath(workspace, 'main.tarn');
    const original = fs.readFileSync(file.fsPath, 'utf8');
    const extension = vscode.extensions.getExtension('tarn-lang.tarn');
    assert.ok(extension, 'development extension must be registered');
    const binary = path.resolve(extension.extensionPath, '../../target/debug/tarn-lsp');
    await vscode.workspace.getConfiguration('tarn').update('serverPath', binary, vscode.ConfigurationTarget.Workspace);
    const document = await vscode.workspace.openTextDocument(file);
    assert.equal(document.languageId, 'tarn');
    await vscode.window.showTextDocument(document);
    await extension.activate();
    assert.ok(extension.isActive);
    const edits = await until(async () => {
        const result = await vscode.commands.executeCommand('vscode.executeFormatDocumentProvider', file, { tabSize: 4, insertSpaces: true });
        return result?.length ? result : undefined;
    }, 'registered formatter');
    assert.ok(edits.length > 0); // VS Code may minimize the full-document edit.
    const edit = new vscode.WorkspaceEdit();
    edit.set(file, edits);
    assert.equal(await vscode.workspace.applyEdit(edit), true);
    assert.equal(document.getText(), 'fn main() { print("😀") }\n');
    assert.equal(fs.readFileSync(file.fsPath, 'utf8'), original);
    const invalid = new vscode.WorkspaceEdit();
    invalid.replace(file, new vscode.Range(0, 0, document.lineCount, 0), 'fn main() { print(missing) }\n');
    await vscode.workspace.applyEdit(invalid);
    await until(() => vscode.languages.getDiagnostics(file).some(d => d.code === 'E2001'), 'compiler diagnostics in editor');
    await vscode.workspace.getConfiguration('tarn').update('lsp.enabled', false, vscode.ConfigurationTarget.Workspace);
    await until(() => vscode.languages.getDiagnostics(file).length === 0, 'diagnostics cleared on stop');
    await vscode.workspace.getConfiguration('tarn').update('lsp.enabled', true, vscode.ConfigurationTarget.Workspace);
    await until(() => vscode.languages.getDiagnostics(file).some(d => d.code === 'E2001'), 'server starts again without reloading');
    await vscode.commands.executeCommand('tarn.restartServer');
    await until(() => vscode.languages.getDiagnostics(file).some(d => d.code === 'E2001'), 'restart command');
    assert.equal(fs.readFileSync(file.fsPath, 'utf8'), original);
    console.log('PASS: real extension activation, document formatting, unsaved diagnostics, disable/enable and restart');
}
module.exports = { run: async () => {
    const result = vscode.Uri.joinPath(vscode.workspace.workspaceFolders[0].uri, 'test-result.json').fsPath;
    try { await run(); fs.writeFileSync(result, JSON.stringify({ ok: true })); }
    catch (error) { fs.writeFileSync(result, JSON.stringify({ ok: false, error: error.stack })); throw error; }
} };
