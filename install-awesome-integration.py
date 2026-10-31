#!/usr/bin/env python3
"""Install widget and replace only the MPD section in the user's theme."""
import argparse, datetime, os, pathlib, re, shutil, stat, tempfile

def patch_theme(text, block):
    starts=list(re.finditer(r'^-- MPD(?:[: ].*)?$',text,re.M))
    if len(starts)!=1: raise ValueError('Expected exactly one -- MPD section; no files changed')
    end=re.search(r'^-- MEM\s*$',text[starts[0].end():],re.M)
    if not end: raise ValueError('Cannot find -- MEM after -- MPD; no files changed')
    return text[:starts[0].start()]+block.rstrip()+'\n\n'+text[starts[0].end()+end.start():]

def replace(path,data,mode):
    with tempfile.NamedTemporaryFile(dir=path.parent,delete=False) as f:
        tmp=pathlib.Path(f.name);f.write(data)
    try:tmp.chmod(mode);tmp.replace(path)
    finally:tmp.unlink(missing_ok=True)

def main():
    parser=argparse.ArgumentParser(description='Update the MPD widget and only its section in theme.lua; preserve backups')
    parser.add_argument('widget',type=pathlib.Path,help='Existing lain/widget/mpd.lua')
    parser.add_argument('--theme',type=pathlib.Path,default=pathlib.Path.home()/'.config/awesome/themes/sigil/theme.lua')
    args=parser.parse_args();root=pathlib.Path(__file__).resolve().parent/'integrations/awesome'
    widget=args.widget.expanduser().resolve(strict=True);theme=args.theme.expanduser().resolve(strict=True)
    if widget==theme:raise ValueError('Widget and theme must be different files')
    old_widget=widget.read_bytes();old_theme=theme.read_bytes()
    if b'lain.widget.mpd' not in old_widget:raise ValueError('Selected file is not the supplied lain MPD widget')
    new_theme=patch_theme(old_theme.decode(),(root/'mpd-theme-section.lua').read_text()).encode()
    files=[(widget,old_widget,(root/'mpd.lua').read_bytes()),(theme,old_theme,new_theme)]
    stamp=datetime.datetime.now().strftime('%Y%m%d-%H%M%S')+f'-{os.getpid()}'
    modes={p:stat.S_IMODE(p.stat().st_mode) for p,_,_ in files}
    for path,_,_ in files:
        backup=path.with_name(path.name+'.backup-'+stamp);shutil.copy2(path,backup);print('Резервная копия:',backup)
    changed=[]
    try:
        for path,old,new in files:
            replace(path,new,modes[path]);changed.append((path,old))
    except Exception:
        for path,old in reversed(changed):replace(path,old,modes[path])
        raise
    print('Обновлены апплет и только блок MPD в теме. Перезапустите Awesome.')
if __name__=='__main__':main()
