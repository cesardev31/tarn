"""Deterministic TLS integration; no external services or credentials."""
import http.server
import json
import os
import pathlib
import ssl
import subprocess
import sys
import tempfile
import threading
import time

class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def handle(self):
        try:
            super().handle()
        except (ConnectionResetError, BrokenPipeError, ssl.SSLError):
            pass

    def do_GET(self):
        if self.path == '/slow':
            time.sleep(.5)
        status = 302 if self.path == '/redirect' else 429 if self.path == '/status' else 200
        self.send_response(status)
        if status == 302:
            self.send_header('Location', '/must-not-follow')
        body = b'{"ok":true}'
        if self.path == '/api/v1/accounts/1/conversations/7':
            assert self.headers['api_access_token'] == 'fixture-chatwoot'
            body = json.dumps({'additional_attributes': {'mail_subject': 'Ayuda café'}}).encode()
        elif self.path == '/api/v1/accounts/1/conversations/8':
            body = json.dumps({'additional_attributes': {'mail_subject': 'Bounded ticket'}}).encode()
        elif self.path == '/api/v1/accounts/1/conversations/8/messages':
            body = json.dumps({'payload': [
                {'created_at': i, 'private': False, 'message_type': 0, 'content': str(i) + 'é' * 2001}
                for i in reversed(range(25))
            ]}).encode()
        elif self.path == '/api/v1/accounts/1/conversations/9':
            body = b'{}'
        elif self.path == '/api/v1/accounts/1/conversations/9/messages':
            body = json.dumps({'payload': [{'created_at': 1, 'private': False,
                                           'message_type': 1, 'content': 'agent only'}]}).encode()
        elif self.path == '/api/v1/accounts/1/conversations/7/messages':
            assert self.headers['api_access_token'] == 'fixture-chatwoot'
            body = json.dumps({'payload': [
                {'created_at': 20, 'private': False, 'message_type': 1, 'content': 'Respuesta'},
                {'created_at': 10, 'private': False, 'message_type': 0, 'content': 'Cliente 日本'},
                {'created_at': 5, 'private': True, 'message_type': 0, 'content': 'PRIVATE'},
                {'created_at': 30, 'private': False, 'message_type': 2, 'content': 'SYSTEM'},
                {'created_at': 40, 'private': False, 'message_type': 0, 'content': '  '},
            ]}).encode()
        self.send_header('Content-Length', '100' if self.path == '/truncated' else str(len(body)))
        self.end_headers()
        if self.path == '/must-not-follow':
            raise AssertionError('redirect followed')
        try:
            self.wfile.write(body)
        except (BrokenPipeError, ssl.SSLError):
            pass

    def do_POST(self):
        assert self.headers['Content-Type'] == 'application/json'
        body = self.rfile.read(int(self.headers['Content-Length']))
        if self.path == '/v1/systemone':
            assert self.headers['Authorization'] == 'Bearer fixture-typesafe'
            request = json.loads(body)
            assert request['model'] == 'jev-latest'
            if request['state']['ticket']['subject'] == 'Bounded ticket':
                messages = request['state']['ticket']['messages']
                assert len(messages) == 20
                for i, message in enumerate(messages, start=5):
                    assert message['from'] == 'cliente'
                    assert message['text'].startswith(str(i))
                    assert len(message['text']) == 2000
            else:
                assert request['state']['ticket'] == {
                'subject': 'Ayuda café',
                'messages': [{'from': 'cliente', 'text': 'Cliente 日本'},
                             {'from': 'agente', 'text': 'Respuesta'}],
            }, request
            questions = request['questions']
            assert set(questions) == {'categoria', 'urgencia', 'molesto'}
            assert questions['categoria']['criteria'] == {'otro': 'Other support requests'}
            assert len(questions['urgencia']['criteria']) == 4
            body = json.dumps({'answers': {
                'categoria': {'type': 'choice', 'choice': 'otro', 'confidence': .8, 'probabilities': {'otro': 1.0}},
                'urgencia': {'type': 'score', 'score': 1.5, 'confidence': .7},
                'molesto': {'type': 'noul', 'noul': .1},
            }, 'usage': {'input_tokens': 100}}).encode()
        else:
            assert self.path == '/echo'
            assert self.headers['api_access_token'] == 'fixture'
        self.send_response(201)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

with tempfile.TemporaryDirectory(prefix='tarn-https-') as directory:
    root = pathlib.Path(directory)
    key, cert = root / 'key.pem', root / 'cert.pem'
    subprocess.run(['openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes',
                    '-keyout', str(key), '-out', str(cert), '-days', '1', '-subj', '/CN=localhost',
                    '-addext', 'subjectAltName=DNS:localhost'], check=True, capture_output=True)
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(cert, key)
    server.socket = context.wrap_socket(server.socket, server_side=True)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    port = server.server_port
    try:
        result = subprocess.run([sys.argv[1], f'https://localhost:{port}', str(cert),
                                 f'https://127.0.0.1:{port}/ok'], capture_output=True, timeout=30)
        assert result.returncode == 0, (result.returncode, result.stdout, result.stderr)
        assert result.stdout == b'HTTPS integration fixture\n', result.stdout
        if len(sys.argv) > 2:
            categories = root / 'categories.json'
            categories.write_text('{"otro":"Other support requests"}')
            env = dict(os.environ, CHATWOOT_TOKEN='fixture-chatwoot', TYPESAFE_API_KEY='fixture-typesafe',
                       TARN_CA_FILE=str(cert), TYPESAFE_ENDPOINT=f'https://localhost:{port}/v1/systemone')
            classified = subprocess.run([sys.argv[2], f'https://localhost:{port}', '1', '7', str(categories)],
                                        env=env, capture_output=True, timeout=30)
            assert classified.returncode == 0, (classified.returncode, classified.stdout, classified.stderr)
            result = json.loads(classified.stdout)
            assert result['answers']['categoria']['choice'] == 'otro'
            assert result['answers']['urgencia']['score'] == 1.5
            assert result['answers']['molesto']['noul'] == .1
            bounded = subprocess.run([sys.argv[2], f'https://localhost:{port}', '1', '8', str(categories)],
                                     env=env, capture_output=True, timeout=30)
            assert bounded.returncode == 0, (bounded.returncode, bounded.stdout, bounded.stderr)
            assert json.loads(bounded.stdout)['answers']['categoria']['choice'] == 'otro'
            empty = subprocess.run([sys.argv[2], f'https://localhost:{port}', '1', '9', str(categories)],
                                   env=env, capture_output=True, timeout=30)
            assert empty.returncode == 1 and b'code -6' in empty.stderr, (empty.returncode, empty.stderr)
            print('Chatwoot → bounded sorted ticket → TypeSafe → lossless JSON: passed')
        print('TLS certificate/hostname, GET/POST, status, redirect, limit, timeout, truncation: passed')
    finally:
        server.shutdown()
        server.server_close()
