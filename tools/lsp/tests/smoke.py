#!/usr/bin/env python3
"""Exercise the actual stdio server, unsaved modules, and navigation."""
import json
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]

def exchange(messages):
    data = b''
    for message in messages:
        body = json.dumps({'jsonrpc': '2.0', **message}).encode()
        data += f'Content-Length: {len(body)}\r\n\r\n'.encode() + body
    result = subprocess.run([str(ROOT / 'target/debug/tarn-lsp')], input=data, capture_output=True, timeout=30)
    assert result.returncode == 0, result.stderr.decode()
    output = result.stdout
    parsed = []
    while output:
        header, output = output.split(b'\r\n\r\n', 1)
        size = int(header.split(b':', 1)[1])
        parsed.append(json.loads(output[:size]))
        output = output[size:]
    return parsed

def opened(uri, text, version=1):
    return {'method': 'textDocument/didOpen', 'params': {'textDocument': {'uri': uri, 'languageId': 'tarn', 'version': version, 'text': text}}}

def changed(uri, text, version):
    return {'method': 'textDocument/didChange', 'params': {'textDocument': {'uri': uri, 'version': version}, 'contentChanges': [{'text': text}]}}

def request(method, ident, uri, line, character):
    return {'id': ident, 'method': method, 'params': {'textDocument': {'uri': uri}, 'position': {'line': line, 'character': character}}}

with tempfile.TemporaryDirectory(prefix='tarn lsp ñ ') as directory:
    root = Path(directory)
    entry = root / 'main.tarn'
    entry.write_text('fn main() {}\n')
    uri = entry.as_uri()
    bad = 'fn main() {\n    print("😀", missing)\n}\n'
    good = 'fn main() {\n    value := 42\n    print(value)\n}\n'
    messages = [
        {'id': 1, 'method': 'initialize', 'params': {}},
        opened(uri, bad), changed(uri, good, 2),
        request('textDocument/hover', 2, uri, 2, 11),
        request('textDocument/definition', 3, uri, 2, 11),
        {'method': 'textDocument/didClose', 'params': {'textDocument': {'uri': uri}}},
        {'id': 4, 'method': 'shutdown'}, {'method': 'exit'},
    ]
    out = exchange(messages)
    responses = {m['id']: m for m in out if 'id' in m}
    assert responses[1]['result']['capabilities']['textDocumentSync']['change'] == 1
    diagnostics = [m['params'] for m in out if m.get('method') == 'textDocument/publishDiagnostics']
    error = next(d for d in diagnostics[0]['diagnostics'] if d['code'] == 'E2001')
    assert error['range']['start'] == {'line': 1, 'character': 16}, error
    assert diagnostics[1]['diagnostics'] == [], diagnostics[1]
    assert diagnostics[1]['version'] == 2
    assert responses[2]['result']['contents']['value'] == 'value: i64', responses[2]
    assert responses[3]['result']['uri'] == uri, responses[3]
    assert responses[3]['result']['range']['start'] == {'line': 1, 'character': 4}
    assert diagnostics[-1]['diagnostics'] == []
    assert entry.read_text() == 'fn main() {}\n', 'editor buffers must not modify disk'

    child = root / 'util.tarn'
    child.write_text('pub fn get() i32 { return 1 }\n')
    main = 'import "util"\nfn main() {\n    value := util.get()\n    print(value)\n}\n'
    out = exchange([
        {'id': 1, 'method': 'initialize', 'params': {}}, opened(uri, main),
        opened(child.as_uri(), 'pub fn get() i32 { return unknown }\n'),
        changed(child.as_uri(), 'pub fn get() i32 { return 2 }\n', 2),
        request('textDocument/definition', 2, uri, 2, 19),
        {'id': 3, 'method': 'shutdown'}, {'method': 'exit'},
    ])
    child_ds = [m['params']['diagnostics'] for m in out if m.get('method') == 'textDocument/publishDiagnostics' and m['params']['uri'] == child.as_uri()]
    assert any(d['code'] == 'E2001' for d in child_ds[0]), child_ds
    assert child_ds[-1] == [], child_ds
    definition = next(m for m in out if m.get('id') == 2)['result']
    assert definition['uri'] == child.as_uri(), definition
    assert child.read_text() == 'pub fn get() i32 { return 1 }\n'
print('PASS: stdio lifecycle, unsaved diagnostics/clearing, UTF-16, hover, definition, imported buffers, unchanged disk')

# Async source facts are visible even while executable frame lowering is gated.
with tempfile.TemporaryDirectory(prefix='tarn async lsp ') as directory:
    entry = Path(directory) / 'main.tarn'
    entry.write_text('fn main() {}\n')
    uri = entry.as_uri()
    source = 'async fn get() i32 { return 42 }\nfn main() {\n    op := get()\n    op\n}\n'
    out = exchange([
        {'id': 1, 'method': 'initialize', 'params': {}}, opened(uri, source),
        request('textDocument/hover', 2, uri, 3, 5),
        request('textDocument/definition', 3, uri, 2, 11),
        changed(uri, 'fn main() { await 42 }\n', 2),
        {'id': 4, 'method': 'shutdown'}, {'method': 'exit'},
    ])
    diagnostics = [m['params']['diagnostics'] for m in out if m.get('method') == 'textDocument/publishDiagnostics']
    assert [d['code'] for d in diagnostics[0]] == ['E3062'], diagnostics
    assert diagnostics[0][0]['range']['start'] == {'line': 0, 'character': 9}
    assert [d['code'] for d in diagnostics[1]] == ['E3060'], diagnostics
    responses = {m['id']: m for m in out if 'id' in m}
    assert responses[2]['result']['contents']['value'] == 'op: async computation<i32>', responses[2]
    assert responses[3]['result']['range']['start'] == {'line': 0, 'character': 9}, responses[3]
    assert entry.read_text() == 'fn main() {}\n'
print('PASS: async source types, definition, context diagnostics and explicit lowering gate')

# Official stdlib buffers keep their declaration identities, including intrinsics.
# A user file named net.tarn must not acquire that authority.
for module in ('core', 'net', 'string'):
    entry = ROOT / 'stdlib' / module / f'{module}.tarn'
    text = entry.read_text()
    out = exchange([{'id': 1, 'method': 'initialize', 'params': {}},
                    opened(entry.as_uri(), text),
                    {'id': 2, 'method': 'shutdown'}, {'method': 'exit'}])
    diagnostics = [m['params']['diagnostics'] for m in out if m.get('method') == 'textDocument/publishDiagnostics']
    assert diagnostics and diagnostics[-1] == [], (module, diagnostics)
    assert entry.read_text() == text

with tempfile.TemporaryDirectory(prefix='tarn untrusted stdlib ') as directory:
    entry = Path(directory) / 'net.tarn'
    text = 'extern "intrinsic" fn injected() i32\nfn main() {}\n'
    entry.write_text(text)
    out = exchange([{'id': 1, 'method': 'initialize', 'params': {}},
                    opened(entry.as_uri(), text),
                    {'id': 2, 'method': 'shutdown'}, {'method': 'exit'}])
    diagnostics = [m['params']['diagnostics'] for m in out if m.get('method') == 'textDocument/publishDiagnostics']
    assert any(d.get('code') == 'E2027' for d in diagnostics[-1]), diagnostics
net = ROOT / 'stdlib/net/net.tarn'
text = net.read_text()
out = exchange([{'id': 1, 'method': 'initialize', 'params': {}},
                opened(net.as_uri(), text),
                changed(net.as_uri(), text + '\nfn lsp_probe() { missing_lsp_value }\n', 2),
                changed(net.as_uri(), text, 3),
                {'id': 2, 'method': 'shutdown'}, {'method': 'exit'}])
diagnostics = [m['params']['diagnostics'] for m in out if m.get('method') == 'textDocument/publishDiagnostics']
assert any(d.get('code') == 'E2001' for d in diagnostics[1]), diagnostics
assert diagnostics[-1] == [] and net.read_text() == text

with tempfile.TemporaryDirectory(prefix='tarn stdlib overlay ') as directory:
    entry = Path(directory) / 'main.tarn'
    main = 'import "net"\nfn main() { print(net.lsp_probe()) }\n'
    entry.write_text(main)
    out = exchange([{'id': 1, 'method': 'initialize', 'params': {}},
                    opened(net.as_uri(), text + '\npub fn lsp_probe() i32 { return 42 }\n'),
                    opened(entry.as_uri(), main),
                    {'id': 2, 'method': 'shutdown'}, {'method': 'exit'}])
    diagnostics = [m['params']['diagnostics'] for m in out if m.get('method') == 'textDocument/publishDiagnostics' and m['params']['uri'] == entry.as_uri()]
    assert diagnostics and diagnostics[-1] == [], diagnostics
    assert net.read_text() == text
print('PASS: official stdlib buffers, live errors, imported overlays and untrusted intrinsic rejection')
