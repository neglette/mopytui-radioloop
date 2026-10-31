"""GStreamer pipe transport regression tests, no internet or audio device needed."""
import json
import os
from pathlib import Path
import select
import subprocess
import tempfile
import time
import unittest
import wave

BACKEND = Path(__file__).resolve().parents[1] / 'src/radio/gst_backend.py'

class RadioTransportTest(unittest.TestCase):
    def test_proxy_pipe_keeps_decoding_across_pause_without_range_requests(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with wave.open(str(root / 'audio.wav'), 'wb') as out:
                out.setnchannels(2); out.setsampwidth(2); out.setframerate(44100)
                out.writeframes(b'\0' * (44100 * 4 * 12))
            curl = root / 'curl'
            curl.write_text('''#!/usr/bin/python3
import json, os, sys, time
from pathlib import Path
root = Path(__file__).parent
(root/'arguments.json').write_text(json.dumps(sys.argv[1:]))
(root/'pid').write_text(str(os.getpid()))
try:
    with open(root/'audio.wav','rb') as stream:
        while chunk := stream.read(4096):
            sys.stdout.buffer.write(chunk); sys.stdout.buffer.flush()
            time.sleep(.002)
except BrokenPipeError:
    pass
''')
            curl.chmod(0o700)
            env = dict(os.environ, PATH=f'{root}:/usr/bin:/bin', MOPYTUI_RADIO_PROXY='socks5h://127.0.0.1:1080')
            source = BACKEND.read_text().replace("'autoaudiosink'", "'fakesink sync=true'")
            child = subprocess.Popen(['python3', '-u', '-c', source, 'https://example.test/radio', 'play'],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
            events = []; paused = resumed = False
            start = time.monotonic()
            try:
                while time.monotonic() - start < 6:
                    elapsed = time.monotonic() - start
                    command = {'volume': .5}
                    if elapsed >= 2 and not paused:
                        command['paused'] = True; paused = True
                    if elapsed >= 3 and not resumed:
                        command['paused'] = False; resumed = True
                    child.stdin.write((json.dumps(command)+'\n').encode()); child.stdin.flush()
                    readable, _, _ = select.select([child.stdout], [], [], .05)
                    if readable:
                        line = child.stdout.readline()
                        self.assertTrue(line, "audio backend exited unexpectedly")
                        event = json.loads(line)
                        self.assertIn(event['event'], ('audio', 'buffering'), event)
                        if event['event'] == 'audio': events.append(elapsed)
                child.stdin.write(b'{"stop":true}\n'); child.stdin.flush()
                child.wait(timeout=3)
                self.assertGreater(len(events), 20)
                self.assertLess(max(b-a for a,b in zip(events, events[1:])), 1.)
                args = json.loads((root/'arguments.json').read_text())
                self.assertEqual(args[args.index('--proxy')+1], 'socks5h://127.0.0.1:1080')
                self.assertEqual(args[args.index('--retry')+1], '0')
                self.assertNotIn('--range', args)
                self.assertNotIn('--continue-at', args)
                pid = int((root/'pid').read_text())
                with self.assertRaises(ProcessLookupError): os.kill(pid, 0)
            finally:
                if child.poll() is None: child.kill()
                child.communicate()

if __name__ == '__main__': unittest.main()
