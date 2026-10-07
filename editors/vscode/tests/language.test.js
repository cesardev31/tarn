const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const root = path.join(__dirname, '..');
const read = file => JSON.parse(fs.readFileSync(path.join(root, file), 'utf8'));
const grammar = read('syntaxes/tarn.tmLanguage.json');

test('manifest references shipped assets and registered languages', () => {
    const manifest = read('package.json');
    const languages = new Set(manifest.contributes.languages.map(l => l.id));
    for (const language of manifest.contributes.languages) {
        assert.ok(fs.existsSync(path.join(root, language.configuration)));
        for (const icon of Object.values(language.icon)) assert.ok(fs.existsSync(path.join(root, icon)));
    }
    for (const item of [...manifest.contributes.grammars, ...manifest.contributes.snippets]) {
        assert.ok(languages.has(item.language));
        assert.ok(fs.existsSync(path.join(root, item.path)));
        read(item.path);
    }
    assert.ok(manifest.activationEvents.includes('onLanguage:tarn'));
    assert.equal(read('language-configuration.json').comments.lineComment, '//');
});

test('grammar distinguishes numeric forms and highlights current method syntax', () => {
    const [float, integer] = grammar.repository.numbers.patterns.map(p => new RegExp(p.match));
    for (const literal of ['42', '0xff', '0o77', '0b1010']) { assert.equal(float.test(literal), false, literal); assert.ok(integer.test(literal)); }
    for (const literal of ['1.25', '1e3', '1.25e-3', '1_000.5']) assert.ok(float.test(literal), literal);
    const declaration = new RegExp(grammar.repository.declarations.patterns[0].match);
    for (const [source, name] of [['fn hello()', 'hello'], ['fn Pair<A, B>.first(&self)', 'first'], ['pub async fn run()', 'run'], ['unsafe extern "C" fn write(', 'write']]) assert.equal(declaration.exec(source)[4], name);
    const contextual = new RegExp(grammar.repository.keywords.patterns[1].match);
    for (const source of ['scope {', 'any Writer', 'once fn()', 'borrows(a)']) assert.ok(contextual.test(source), source);
    for (const source of ['scope := 1', 'once := 1', 'any := 1']) assert.equal(contextual.test(source), false, source);
});

test('snippets use Tarn statements, parameter types and explicit returns', () => {
    const snippets = read('snippets/tarn.json');
    for (const snippet of Object.values(snippets)) {
        assert.ok(Array.isArray(snippet.body));
        assert.equal(snippet.body.join('\n').includes(';'), false);
    }
    assert.ok(snippets.Test.body[0].startsWith('fn test_'));
    assert.equal(snippets.Struct.body[1].includes(':'), true); // Placeholder colons only.
});

test('snippet defaults parse through the real Tarn frontend', () => {
    const os = require('node:os');
    const { spawnSync } = require('node:child_process');
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'tarn-snippets-'));
    const binary = path.resolve(root, '../../target/debug/tarn');
    try {
        for (const [name, snippet] of Object.entries(read('snippets/tarn.json'))) {
            let source = snippet.body.join('\n').replace(/\$\{\d+:([^}]*)\}/g, '$1').replace(/\$\{\d+\}|\$\d+/g, '');
            if (name === 'For' || name === 'Match') source = `fn main() {\n${source}\n}`;
            const file = path.join(directory, `${name}.tarn`);
            fs.writeFileSync(file, source);
            const result = spawnSync(binary, ['ast', file], { encoding: 'utf8' });
            assert.equal(result.status, 0, `${name}: ${result.error || result.stderr}`);
        }
    } finally { fs.rmSync(directory, { recursive: true, force: true }); }
});
