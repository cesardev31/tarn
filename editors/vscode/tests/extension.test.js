const test = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');

function fixture(enabled = true) {
    const events = [], commands = new Map(), settings = { 'lsp.enabled': enabled, serverPath: '' };
    let changed, failStart = false;
    const vscode = {
        workspace: {
            workspaceFolders: [{ uri: { fsPath: '/tmp/tarn external project' } }],
            getConfiguration: () => ({ get: (key, fallback) => settings[key] ?? fallback }),
            createFileSystemWatcher: () => { events.push('watch'); return { dispose() { events.push('unwatch'); } }; },
            onDidChangeConfiguration: callback => { changed = callback; return { dispose() {} }; },
        },
        commands: { registerCommand: (name, callback) => { commands.set(name, callback); return { dispose() {} }; } },
        window: { showErrorMessage: text => { events.push(['error', text]); } },
    };
    class LanguageClient {
        constructor(id, name, options, clientOptions) { events.push(['client', options, clientOptions]); }
        async start() { events.push('start'); if (failStart) throw new Error('missing executable'); }
        async stop() { events.push('stop'); }
    }
    const module = { exports: {} };
    vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../extension.js'), 'utf8'), {
        module, process: { cwd: () => '/tmp' },
        require: name => {
            if (name === 'vscode') return vscode;
            if (name === 'vscode-languageclient/node') { events.push('require-client'); return { LanguageClient }; }
            if (name === 'fs') return { constants: fs.constants, accessSync() { throw new Error('missing'); } };
            return require(name);
        },
    });
    return { extension: module.exports, context: { extensionPath: '/tmp/installed/extensions/tarn', subscriptions: [] }, commands, events, settings,
        async change(key) { await changed({ affectsConfiguration: candidate => candidate === key }); },
        fail(value) { failStart = value; },
    };
}

test('syntax-only activation can enable, disable and restart without a reload', async () => {
    const f = fixture(false);
    await f.extension.activate(f.context);
    assert.equal(f.commands.has('tarn.restartServer'), true);
    assert.deepEqual(f.events, []);
    f.settings['lsp.enabled'] = true;
    await f.change('tarn.lsp.enabled');
    assert.ok(f.events.includes('start'));
    f.settings['lsp.enabled'] = false;
    await f.change('tarn.lsp.enabled');
    assert.equal(f.events.filter(e => e === 'stop').length, 1);
    assert.equal(f.events.filter(e => e === 'unwatch').length, 1);
    const count = f.events.length;
    await f.commands.get('tarn.restartServer')();
    assert.equal(f.events.length, count);
    f.settings['lsp.enabled'] = true;
    await f.change('tarn.lsp.enabled');
    await f.extension.deactivate();
    assert.equal(f.events.filter(e => e === 'stop').length, 2);
    assert.equal(f.events.filter(e => e === 'watch').length, 2);
    assert.equal(f.events.filter(e => e === 'unwatch').length, 2);
});

test('configured relative paths use the external workspace and restart serially', async () => {
    const f = fixture();
    f.settings.serverPath = 'bin/tarn-lsp';
    await f.extension.activate(f.context);
    const options = f.events.find(e => Array.isArray(e) && e[0] === 'client')[1];
    assert.equal(options.command, '/tmp/tarn external project/bin/tarn-lsp');
    assert.equal(options.options.cwd, '/tmp/tarn external project');
    assert.deepEqual(Array.from(options.args), []);
    await Promise.all([f.commands.get('tarn.restartServer')(), f.commands.get('tarn.restartServer')()]);
    assert.deepEqual(f.events.filter(e => e === 'start' || e === 'stop'), ['start', 'stop', 'start', 'stop', 'start']);
    await f.extension.deactivate();
});

test('startup failures release resources and can recover on restart', async () => {
    const f = fixture();
    f.fail(true);
    await f.extension.activate(f.context);
    assert.equal(f.events.filter(e => e === 'unwatch').length, 1);
    assert.ok(f.events.some(e => Array.isArray(e) && e[0] === 'error' && e[1].includes('tarn.serverPath')));
    f.fail(false);
    await f.commands.get('tarn.restartServer')();
    await f.extension.deactivate();
    assert.equal(f.events.filter(e => e === 'unwatch').length, 2);
});
