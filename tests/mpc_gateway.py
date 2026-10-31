"""Exercise the actual mpc client against the gateway and isolated fake servers.
Run after cargo build: python3 tests/mpc_gateway.py /path/to/mopytui-mpd-proxy
Requires loopback TCP + Unix sockets; never contacts the user's Mopidy.
"""
import json
import os
from pathlib import Path
import socket
import socketserver
import subprocess
import sys
import tempfile
import threading
import time
import unittest

BINARY = str(Path(sys.argv.pop(1)).resolve()) if len(sys.argv) > 1 else str(Path(__file__).resolve().parents[1]/'target/debug/mopytui-mpd-proxy')

class GatewayTest(unittest.TestCase):
    def test_mpc_controls_and_closed_tui_fallback(self):
        state = {'active': True, 'paused': False, 'volume': 80, 'index': 0}
        radio_commands = []; mopidy_commands = []
        guard = threading.Lock()
        status = lambda: [f"volume: {state['volume']}", 'repeat: 0', 'random: 0', 'single: 0', 'consume: 0',
            'playlist: 1', 'playlistlength: 2', 'state: '+('pause' if state['paused'] else 'play'),
            f"song: {state['index']}", f"songid: {state['index']+1}", 'elapsed: 10.000']
        class Radio(socketserver.StreamRequestHandler):
            def handle(self):
                command = self.rfile.readline().decode().strip()
                args = command.split(); name = args[0]; lines=[]; handled=True
                with guard:
                    radio_commands.append(command)
                    if not state['active']: handled=False
                    elif name=='status': lines=status()
                    elif name=='currentsong': lines=['file: https://example.test/live', 'Title: Test radio', 'Name: Test station', 'Artist: Test artist', 'Comment: mopytui-radio', f"Pos: {state['index']}", f"Id: {state['index']+1}"]
                    elif name=='setvol': state['volume']=int(args[1].strip('"'))
                    elif name=='volume': state['volume']+=int(args[1].strip('"'))
                    elif name=='next': state['index']=(state['index']+1)%2
                    elif name=='previous': state['index']=(state['index']-1)%2
                    elif name=='play': state['paused']=False
                    elif name=='pause': state['paused']=args[1].strip('"')=='1' if len(args)>1 else not state['paused']
                    elif name in ('seek','seekid','seekcur'): pass
                    elif name=='stop': state['active']=False
                    else: handled=False
                self.wfile.write((json.dumps({'handled':handled,'lines':lines,'error':None})+'\n').encode())
        class Mopidy(socketserver.StreamRequestHandler):
            def handle(self):
                self.wfile.write(b'OK MPD 0.23.5\n')
                while line:=self.rfile.readline():
                    command=line.decode().strip(); name=command.split()[0]
                    with guard: mopidy_commands.append(command)
                    if name=='commands':
                        self.wfile.write(b'command: status\ncommand: currentsong\ncommand: next\ncommand: previous\ncommand: setvol\ncommand: pause\ncommand: play\ncommand: seekcur\n')
                    elif name=='status': self.wfile.write(b'volume: 44\nrepeat: 0\nrandom: 0\nsingle: 0\nconsume: 0\nplaylist: 1\nplaylistlength: 0\nstate: stop\n')
                    self.wfile.write(b'OK\n')
        class UnixServer(socketserver.ThreadingUnixStreamServer): daemon_threads=True
        class TcpServer(socketserver.ThreadingTCPServer): daemon_threads=True;allow_reuse_address=True
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'radio.sock'
            radio=UnixServer(str(path), Radio)
            use_unix = os.environ.get('MOPYTUI_TEST_UNIX') == '1'
            mopidy=UnixServer(str(Path(tmp)/'mopidy.sock'), Mopidy) if use_unix else TcpServer(('127.0.0.1',0), Mopidy)
            for server in (radio,mopidy): threading.Thread(target=server.serve_forever,daemon=True).start()
            with socket.socket() as free:
                free.bind(('127.0.0.1',0)); port=free.getsockname()[1]
            listen = str(Path(tmp)/'gateway.sock') if use_unix else f'127.0.0.1:{port}'
            upstream = str(Path(tmp)/'mopidy.sock') if use_unix else f'127.0.0.1:{mopidy.server_address[1]}'
            process=subprocess.Popen([BINARY,'--listen',listen,'--upstream',upstream],
                env=dict(os.environ,MOPYTUI_CONTROL_SOCKET=str(path)),stdout=subprocess.PIPE,stderr=subprocess.PIPE)
            def mpc(*args):
                result=subprocess.run(['mpc','-h',listen if use_unix else '127.0.0.1','-p',str(port),*args],capture_output=True,text=True,timeout=5)
                self.assertEqual(result.returncode,0,result.stderr)
                return result.stdout
            try:
                for _ in range(100):
                    try:
                        if use_unix:
                            with socket.socket(socket.AF_UNIX) as probe: probe.connect(listen)
                            break
                        with socket.create_connection(('127.0.0.1',port),timeout=.1): break
                    except OSError: time.sleep(.02)
                self.assertIn('Test radio',mpc('status'))
                self.assertIn('Test station / Test artist / Test radio',mpc('-f','%name% / %artist% / %title%','current'))
                mpc('volume','35');self.assertEqual(state['volume'],35)
                mpc('volume','+5');self.assertEqual(state['volume'],40)
                for delta in [-1, 1, -3, 3]:
                    before=state['volume'];mpc('volume',f'{delta:+d}');self.assertEqual(state['volume'],before+delta)
                mpc('pause');self.assertTrue(state['paused'])
                mpc('play');self.assertFalse(state['paused'])
                mpc('toggle');self.assertTrue(state['paused'])
                mpc('toggle');self.assertFalse(state['paused'])
                mpc('next');self.assertEqual(state['index'],1)
                mpc('prev');self.assertEqual(state['index'],0)
                mpc('seek','+10')
                self.assertTrue(any(c.startswith('seek') for c in radio_commands))
                mutations=('next','previous','play','pause','setvol','volume','seek','seekcur','seekid')
                self.assertFalse(any(c.split()[0] in mutations for c in mopidy_commands),mopidy_commands)
                # Close the TUI socket entirely; the same gateway must still work.
                radio.shutdown();radio.server_close();path.unlink()
                mpc('next');self.assertIn('next',mopidy_commands)
                self.assertIn('44%',mpc('status'))
            finally:
                process.terminate();process.communicate(timeout=3)
                radio.shutdown();radio.server_close()
                mopidy.shutdown();mopidy.server_close()

if __name__=='__main__':unittest.main()
