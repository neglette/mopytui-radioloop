"""Real GStreamer smoke tests: decoded WAV succeeds; HTTP text does not.
Run: python3 tests/gstreamer_probe.py (requires Python GI / Gst).
Set MOPYTUI_HTTP_TEST=1 to additionally test localhost HTTP transport.
"""
import http.server
import json
import os
import pathlib
import subprocess
import tempfile
import threading
import time
import unittest
import wave

BACKEND = pathlib.Path(__file__).resolve().parents[1] / 'src/radio/gst_backend.py'

class ProbeTest(unittest.TestCase):
    def test_local_decoded_audio_and_non_audio(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            with wave.open(str(root / 'tone.wav'), 'wb') as out:
                out.setnchannels(1); out.setsampwidth(2); out.setframerate(8000)
                out.writeframes(b'\x00\x00' * 8000)
            (root / 'not-audio').write_text('not decoded audio')
            for name, expected in [('tone.wav', 'audio'), ('not-audio', 'error')]:
                child = subprocess.Popen(['python3', str(BACKEND), (root / name).as_uri(), 'probe'],
                    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                try:
                    child.wait(timeout=5)
                    output = child.stdout.read().decode()
                    self.assertIn(expected, [json.loads(line)['event'] for line in output.splitlines()], output)
                finally:
                    if child.poll() is None: child.kill()
                    child.communicate()

    @unittest.skipUnless(os.environ.get('MOPYTUI_HTTP_TEST') == '1', 'Set MOPYTUI_HTTP_TEST=1 where loopback TCP is allowed')
    def test_http_decoded_audio_and_non_audio(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            wav = root / 'tone.wav'
            with wave.open(str(wav), 'wb') as out:
                out.setnchannels(1)
                out.setsampwidth(2)
                out.setframerate(8000)
                out.writeframes(b'\x00\x00' * 80000)
            (root / 'not-audio').write_text('HTTP 200 is not proof of audio')
            class Handler(http.server.SimpleHTTPRequestHandler):
                def __init__(self, *args, **kw):
                    super().__init__(*args, directory=tmp, **kw)
                def log_message(self, *args):
                    pass
            server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                for file, expected in [('tone.wav', 'audio'), ('not-audio', 'error')]:
                    with self.subTest(file=file):
                        child = subprocess.Popen(['python3', str(BACKEND),
                            f'http://127.0.0.1:{server.server_port}/{file}', 'probe'],
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            env={k: v for k, v in os.environ.items() if "proxy" not in k.lower()})
                        try:
                            # Keep stdin open: EOF deliberately shuts the backend down.
                            child.wait(timeout=5)
                            output = child.stdout.read().decode()
                            self.assertIn(expected, [json.loads(line)['event'] for line in output.splitlines()], output + child.stderr.read().decode())
                        finally:
                            if child.poll() is None: child.kill()
                            child.communicate()
            finally:
                server.shutdown()
                server.server_close()

    def test_parent_eof_terminates_pipeline(self):
        child = subprocess.Popen(['python3', str(BACKEND), 'file:///nonexistent-mopytui-test-file', 'probe'],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            env={k: v for k, v in os.environ.items() if "proxy" not in k.lower()})
        child.communicate(timeout=3)
        self.assertIsNotNone(child.returncode)

if __name__ == '__main__':
    unittest.main()
