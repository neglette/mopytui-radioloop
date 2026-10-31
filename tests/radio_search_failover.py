"""Exercise the actual search CLI with curl fixtures, no Radio Browser requests."""
import json, os, subprocess, sys, tempfile, time, unittest
from pathlib import Path
BINARY=str(Path(sys.argv.pop(1)).resolve()) if len(sys.argv)>1 else str(Path(__file__).resolve().parents[1]/'target/release/radio-search')
class SearchFailoverTest(unittest.TestCase):
    def search(self, mode):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d)
            (root/'curl').write_text('''#!/usr/bin/python3
import sys,os,json,time
from pathlib import Path
proxy=sys.argv[sys.argv.index('--proxy')+1]
with open(Path(__file__).with_name('calls'),'a') as f:f.write(('proxy' if proxy else 'direct')+'\\n')
if os.environ['SEARCH_TEST_MODE']=='hang' and proxy:time.sleep(30)
if proxy or os.environ['SEARCH_TEST_MODE']=='all503':
    print('curl: (22) The requested URL returned error: 503', file=sys.stderr)
    sys.exit(22)
print(json.dumps([{'stationuuid':'one','name':'Fixture Radio','url':'https://audio.test/live'}]))
''')
            (root/'curl').chmod(0o700)
            start=time.monotonic()
            result=subprocess.run([BINARY,'--json','ambient'],capture_output=True,text=True,timeout=20,env=dict(os.environ,PATH=str(root)+':/usr/bin:/bin',MOPYTUI_RADIO_PROXY='socks5h://localhost:1080',SEARCH_TEST_MODE=mode))
            calls=(root/'calls').read_text().splitlines()
            return result,calls,time.monotonic()-start
    def test_proxy_503_falls_back_and_merges_results(self):
        result,calls,_=self.search('503')
        self.assertEqual(result.returncode,0,result.stderr)
        self.assertEqual(len(json.loads(result.stdout)),1)
        self.assertEqual(calls[:2],['proxy','proxy'])
        self.assertIn('direct',calls)
    def test_all_servers_down_finishes_with_explicit_error(self):
        result,_,elapsed=self.search('all503')
        self.assertNotEqual(result.returncode,0)
        self.assertIn('503',result.stderr)
        self.assertIn('unavailable',result.stderr)
        self.assertLess(elapsed,15)
    def test_hung_proxy_is_cancelled_before_direct_fallback(self):
        result,calls,elapsed=self.search('hang')
        self.assertEqual(result.returncode,0,result.stderr)
        self.assertIn('direct',calls)
        self.assertLess(elapsed,16)
if __name__=='__main__':unittest.main()
