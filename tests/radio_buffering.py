"""Exercise real GStreamer with a deterministic network stall and no sound output."""
import json, os, select, subprocess, tempfile, time, unittest, wave
from pathlib import Path
BACKEND = Path(__file__).resolve().parents[1] / 'src/radio/gst_backend.py'
class BufferingTest(unittest.TestCase):
    def run_stream(self, seconds):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with wave.open(str(root/'audio.wav'), 'wb') as out:
                out.setnchannels(2); out.setsampwidth(2); out.setframerate(44100)
                out.writeframes(b'\0' * (44100 * 4 * 20))
            (root/'curl').write_text('''#!/usr/bin/python3
import sys,time,pathlib
root=pathlib.Path(__file__).parent
data=(root/'audio.wav').read_bytes()
headers=sys.argv[sys.argv.index('--dump-header')+1]
with open(headers,'w') as f:f.write('HTTP/1.1 200 OK\\r\\n\\r\\n')
sys.stdout.buffer.write(data[:44+44100*4*4]);sys.stdout.buffer.flush()
time.sleep(1.5)
sys.stdout.buffer.write(data[44+44100*4*4:]);sys.stdout.buffer.flush()
time.sleep(30)
''')
            (root/'curl').chmod(0o700)
            source = BACKEND.read_text().replace("'autoaudiosink'", "'fakesink sync=true'")
            child = subprocess.Popen(['/usr/bin/python3','-u','-c',source,'https://example.test/live','play'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, bufsize=0,
                env=dict(os.environ, PATH=str(root)+':/usr/bin:/bin', MOPYTUI_RADIO_PROXY='socks5h://127.0.0.1:1080', MOPYTUI_BUFFER_SECONDS=str(seconds)))
            audio=[]; buffering=[]; start=time.monotonic()
            try:
                while time.monotonic()-start < 9:
                    if select.select([child.stdout],[],[],.1)[0]:
                        line=child.stdout.readline()
                        self.assertTrue(line, "backend exited unexpectedly")
                        event=json.loads(line)
                        self.assertNotEqual(event['event'], 'error', event)
                        if event['event']=='audio': audio.append(time.monotonic()-start)
                        if event['event']=='buffering': buffering.append(event['percent'])
                child.stdin.write(b'{"stop":true}\n');child.wait(timeout=3)
                self.assertGreater(len(audio), 20)
                return max(b-a for a,b in zip(audio,audio[1:])),buffering
            finally:
                if child.poll() is None:child.kill()
                child.communicate()
    def test_native_playbin_also_prerolls_and_starts(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'native.wav'
            with wave.open(str(path),'wb') as out:
                out.setnchannels(2);out.setsampwidth(2);out.setframerate(44100)
                out.writeframes(b'\0'*(44100*4*8))
            env=dict(os.environ, MOPYTUI_BUFFER_SECONDS='3')
            env.pop('MOPYTUI_RADIO_PROXY',None)
            source=BACKEND.read_text().replace("'autoaudiosink'", "'fakesink sync=true'")
            child=subprocess.Popen(['/usr/bin/python3','-u','-c',source,path.as_uri(),'play'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,bufsize=0,env=env)
            events=[];deadline=time.monotonic()+4
            try:
                while time.monotonic()<deadline:
                    if not select.select([child.stdout],[],[],.1)[0]:continue
                    line=child.stdout.readline()
                    self.assertTrue(line)
                    event=json.loads(line);events.append(event)
                    self.assertNotEqual(event['event'],'error',event)
                    if event['event']=='audio':break
                self.assertTrue(any(e['event']=='audio' for e in events),events)
                self.assertTrue(any(e.get('percent')==100 for e in events),events)
                child.stdin.write(b'{"stop":true}\n');child.wait(timeout=3)
            finally:
                if child.poll() is None:child.kill()
                child.communicate()
    def test_three_second_buffer_absorbs_short_network_stall(self):
        plain,_=self.run_stream(0)
        buffered,states=self.run_stream(3)
        self.assertGreater(plain, 1.0, plain)
        self.assertLess(buffered, 0.9, (plain,buffered,states))
        self.assertIn(100, states)
        self.assertTrue(any(p < 100 for p in states))
        print(f'Largest audio gap: unbuffered {plain:.2f}s, buffered {buffered:.2f}s')
if __name__=='__main__':unittest.main()
