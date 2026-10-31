#!/usr/bin/env python3
"""Replace the explicitly selected lain MPD widget, preserving a backup."""
import argparse, datetime, os, pathlib, shutil, stat, subprocess, tempfile

def main():
    parser=argparse.ArgumentParser(description='Install Radio-aware lain MPD widget with a backup')
    parser.add_argument('target',type=pathlib.Path,help='Full path to your existing lain/widget/mpd.lua')
    args=parser.parse_args()
    target=args.target.expanduser().resolve(strict=True)
    source=pathlib.Path(__file__).resolve().parent/'integrations/awesome/mpd.lua'
    original=target.read_text()
    if 'lain.widget.mpd' not in original: raise RuntimeError('This does not look like the supplied lain MPD widget; no changes made')
    if shutil.which('luac'): subprocess.run(['luac','-p',str(source)],check=True)
    backup=target.with_name(target.name+'.backup-'+datetime.datetime.now().strftime('%Y%m%d-%H%M%S')+f'-{os.getpid()}')
    shutil.copy2(target,backup)
    with tempfile.NamedTemporaryFile(dir=target.parent,delete=False) as f:
        tmp=pathlib.Path(f.name); f.write(source.read_bytes())
    try:
        tmp.chmod(stat.S_IMODE(target.stat().st_mode));tmp.replace(target)
    finally: tmp.unlink(missing_ok=True)
    print(f'Установлено: {target}\nРезервная копия: {backup}\nПерезапустите Awesome, чтобы он загрузил обновлённый модуль.')
if __name__=='__main__':main()
