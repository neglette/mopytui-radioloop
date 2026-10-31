"""Real ICY demux/decode, with deterministic curl bytes instead of a network."""
import json, os, pathlib, subprocess, tempfile, unittest, wave
BACKEND = pathlib.Path(__file__).resolve().parents[1]/'src/radio/gst_backend.py'
class MetadataTest(unittest.TestCase):
    def test_icy_metadata_and_audio_arrive_together(self):
        with tempfile.TemporaryDirectory() as directory:
            root=pathlib.Path(directory)
            with wave.open(str(root/'audio.wav'),'wb') as out:
                out.setnchannels(1);out.setsampwidth(2);out.setframerate(8000);out.writeframes(b'\0'*16000*8)
            (root/'curl').write_text('''#!/usr/bin/python3
import sys,time,pathlib
root=pathlib.Path(__file__).parent
headers=sys.argv[sys.argv.index('--dump-header')+1]
with open(headers,'w') as f: f.write('HTTP/1.1 200 OK\\r\\nicy-metaint: 4096\\r\\n\\r\\n')
data=(root/'audio.wav').read_bytes()
for i in range(0,len(data),4096):
    audio=data[i:i+4096];sys.stdout.buffer.write(audio)
    if len(audio)==4096:
        title='Performer - First track' if i < 32768 else 'Second Artist - Second track'
        metadata=("StreamTitle='"+title+"';").encode()
        length=(len(metadata)+15)//16
        sys.stdout.buffer.write(bytes([length])+metadata.ljust(length*16,b'\\0'))
    sys.stdout.buffer.flush();time.sleep(.05)
time.sleep(3)
''');(root/'curl').chmod(0o700)
            source=BACKEND.read_text().replace("'autoaudiosink'", "'fakesink sync=true'")
            child=subprocess.Popen(['/usr/bin/python3','-u','-c',source,'https://example.test/radio','play'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=dict(os.environ,PATH=str(root)+':/usr/bin:/bin',MOPYTUI_RADIO_PROXY='socks5h://127.0.0.1:1080'))
            import select,time
            events=[]; deadline=time.monotonic()+7
            try:
                while time.monotonic()<deadline:
                    if not select.select([child.stdout],[],[],.1)[0]: continue
                    line=child.stdout.readline()
                    if not line: break
                    event=json.loads(line);events.append(event)
                    if any(e.get('title')=='Second track' for e in events) and any(e['event']=='audio' for e in events): break
                child.stdin.write(b'{"stop":true}\n');child.stdin.flush();child.wait(timeout=3)
                self.assertTrue(any(e['event']=='audio' for e in events),events)
                self.assertTrue(any(e.get('artist')=='Performer' and e.get('title')=='First track' for e in events),events)
                self.assertTrue(any(e.get('artist')=='Second Artist' and e.get('title')=='Second track' for e in events),events)
                self.assertFalse(any(e['event']=='error' for e in events),events)
            finally:
                if child.poll() is None:child.kill()
                child.communicate()
if __name__=='__main__':unittest.main()
