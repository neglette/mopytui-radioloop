"""Real Python socket query against a fake MPD over a Unix socket."""
from pathlib import Path
import socket, socketserver, subprocess, tempfile, threading, time, unittest
SCRIPT=(Path(__file__).resolve().parents[1]/'integrations/awesome/mpd.lua').read_text().split('local mpd_query = [=[\n',1)[1].split(']=]',1)[0]
class QueryTest(unittest.TestCase):
    def test_updates_without_waiting_for_server_disconnect(self):
        state={'title':'First'}
        class Handler(socketserver.StreamRequestHandler):
            def handle(self):
                self.wfile.write(b'OK MPD 0.23.5\n')
                while line:=self.rfile.readline():
                    if line.strip()==b'status':self.wfile.write(b'state: play\nOK\n')
                    elif line.strip()==b'currentsong':self.wfile.write(('Name: Station\nArtist: Artist\nTitle: '+state['title']+'\nComment: mopytui-radio\nOK\n').encode())
                    else:self.wfile.write(b'ACK [5@0] {} unknown\n')
        with tempfile.TemporaryDirectory() as d:
            path=str(Path(d)/'mpd.sock')
            server=socketserver.ThreadingUnixStreamServer(path,Handler)
            threading.Thread(target=server.serve_forever,daemon=True).start()
            try:
                timings=[]
                for title in ['First','Second']:
                    state['title']=title;start=time.monotonic()
                    result=subprocess.run(['python3','-c',SCRIPT,path,'6600',''],capture_output=True,text=True,timeout=2)
                    timings.append(time.monotonic()-start)
                    self.assertEqual(result.returncode,0,result.stderr)
                    self.assertIn('Title: '+title,result.stdout)
                    self.assertLess(timings[-1],1.)
                print('Query durations:',timings)
            finally:server.shutdown();server.server_close()
if __name__=='__main__':unittest.main()
