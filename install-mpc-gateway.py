#!/usr/bin/env python3
"""Install the per-user MPD gateway. Run locally, without sudo, with the TUI closed."""
import datetime
import json
import os
from pathlib import Path
import re
import shutil
import socket
import stat
import subprocess
import sys
import tempfile
import time
import tomllib


def set_ini_port(text, port):
    lines = text.splitlines(keepends=True)
    headers = [(i, re.match(r'^\s*\[([^]]+)\]', line)) for i, line in enumerate(lines)]
    headers = [(i, match.group(1).lower()) for i, match in headers if match]
    sections = [i for i, name in headers if name == 'mpd']
    if len(sections) > 1: raise ValueError('Duplicate [mpd] sections; refusing to guess')
    if not sections: return text.rstrip() + f'\n\n[mpd]\nport = {port}\n'
    start = sections[0]; end = next((i for i, _ in headers if i > start), len(lines))
    keys = [i for i in range(start+1, end) if re.match(r'^\s*port\s*=', lines[i])]
    if len(keys) > 1: raise ValueError('Duplicate MPD port settings')
    if keys: lines[keys[0]] = f'port = {port}\n'
    else: lines.insert(start+1, f'port = {port}\n')
    return ''.join(lines)


def set_tui_port(text, port):
    config = tomllib.loads(text)
    if config.get('mpd_port', 6600) not in (6600, 6602):
        raise ValueError('mopytui uses a custom MPD port; refusing to overwrite it')
    lines = text.splitlines(keepends=True)
    end = next((i for i, line in enumerate(lines) if re.match(r'^\s*\[', line)), len(lines))
    key = next((i for i in range(end) if re.match(r'^\s*mpd_port\s*=', lines[i])), None)
    if key is None: lines.insert(0, f'mpd_port = {port}\n')
    else: lines[key] = f'mpd_port = {port}\n'
    result = ''.join(lines)
    assert tomllib.loads(result)['mpd_port'] == port
    return result


def systemctl(*args, check=True):
    return subprocess.run(['systemctl', '--user', *args], check=check, capture_output=True, text=True)


def atomic_write(path, data, mode=0o600):
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as tmp:
        temporary = Path(tmp.name)
        tmp.write(data)
    try:
        temporary.chmod(mode)
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def wait_mpd(port):
    last_error = None
    for _ in range(80):
        try:
            with socket.create_connection(('127.0.0.1', port), timeout=.3) as connection:
                connection.settimeout(.3)
                if connection.recv(128).startswith(b'OK MPD '): return
        except OSError as error: last_error = error
        time.sleep(.1)
    raise RuntimeError(f'MPD did not become available on {port}: {last_error}')


def check_gateway_port(port=6600):
    # Match Tokio's TCP listener: a recently closed connection in TIME_WAIT
    # must not be mistaken for a live listener. SO_REUSEPORT is NOT enabled.
    with socket.socket() as check:
        check.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            check.bind(('127.0.0.1', port))
            check.listen(1)
        except OSError as error:
            raise RuntimeError(f'Cannot listen on gateway port {port}: {error}; rolling back') from error


def quote_unit(value):
    return '"' + str(value).replace('\\', '\\\\').replace('"', '\\"').replace('%', '%%') + '"'


def main():
    if os.geteuid() == 0: raise RuntimeError('Run this as your ordinary user, without sudo')
    home = Path.home(); source = Path(__file__).resolve().parent
    config_home = Path(os.environ.get('XDG_CONFIG_HOME', home/'.config'))
    cache_home = Path(os.environ.get('XDG_CACHE_HOME', home/'.cache'))
    state_home = Path(os.environ.get('XDG_STATE_HOME', home/'.local/state'))
    mopidy = config_home/'mopidy/mopidy.conf'
    tui = config_home/'mopytui/config.toml'
    unit = config_home/'systemd/user/mopytui-mpd-proxy.service'
    binary = home/'.local/lib/mopytui-radio/mopytui-mpd-proxy'
    control = Path(os.environ.get('MOPYTUI_CONTROL_SOCKET', cache_home/'mopytui/radio-control.sock'))
    incoming = source/'mopytui-mpd-proxy'
    if not incoming.is_file(): raise RuntimeError('Unpack the binary archive before running the installer')
    if not mopidy.is_file(): raise RuntimeError(f'Mopidy config was not found: {mopidy}')
    with socket.socket(socket.AF_UNIX) as check:
        try: check.connect(str(control))
        except (FileNotFoundError, ConnectionRefusedError): pass
        else: raise RuntimeError('Close mopytui before installing the gateway')
    if systemctl('is-active', '--quiet', 'mopidy.service', check=False).returncode:
        raise RuntimeError('This installer requires your running user service mopidy.service')
    if not unit.exists():
        with socket.socket() as check:
            check.settimeout(.3)
            if check.connect_ex(('127.0.0.1',6602)) == 0:
                raise RuntimeError('Port 6602 is already in use; nothing was changed')
    mpd_text = set_ini_port(mopidy.read_text(), 6602)
    tui_text = set_tui_port(tui.read_text() if tui.exists() else '', 6602)
    unit_text = f'''[Unit]
Description=mopytui Radio MPD gateway (mpc 6600 -> Mopidy 6602)
After=network.target mopidy.service

[Service]
ExecStart={quote_unit(binary)} --listen 127.0.0.1:6600 --upstream 127.0.0.1:6602
Environment={quote_unit('MOPYTUI_CONTROL_SOCKET='+str(control))}
Restart=on-failure
RestartSec=2

[Install]
WantedBy=default.target
'''
    backup = state_home/'mopytui-radio/backups'/(datetime.datetime.now().strftime('%Y%m%d-%H%M%S')+f'-{os.getpid()}')
    backup.mkdir(parents=True)
    originals = {}
    for index, path in enumerate((mopidy, tui, unit, binary)):
        data = path.read_bytes() if path.exists() else None
        mode = stat.S_IMODE(path.stat().st_mode) if path.exists() else 0o600
        originals[path] = (data, mode)
        if data is not None: (backup/f'{index}-{path.name}').write_bytes(data)
    (backup/'paths.json').write_text(json.dumps({str(i):str(p) for i,p in enumerate(originals)},indent=2))
    was_enabled = systemctl('is-enabled','--quiet','mopytui-mpd-proxy.service',check=False).returncode == 0
    was_active = systemctl('is-active','--quiet','mopytui-mpd-proxy.service',check=False).returncode == 0
    print(f'Резервные копии: {backup}', flush=True)
    try:
        systemctl('stop','mopytui-mpd-proxy.service',check=False)
        atomic_write(binary, incoming.read_bytes(), 0o755)
        atomic_write(mopidy, mpd_text.encode(), originals[mopidy][1])
        atomic_write(tui, tui_text.encode(), originals[tui][1])
        atomic_write(unit, unit_text.encode(), 0o644)
        systemctl('daemon-reload')
        systemctl('restart','mopidy.service')
        wait_mpd(6602)
        check_gateway_port()
        systemctl('enable','--now','mopytui-mpd-proxy.service')
        wait_mpd(6600)
        systemctl("is-active", "--quiet", "mopytui-mpd-proxy.service")
    except Exception:
        systemctl('stop','mopytui-mpd-proxy.service',check=False)
        if not was_enabled: systemctl('disable','mopytui-mpd-proxy.service',check=False)
        for path,(data,mode) in originals.items():
            if data is None: path.unlink(missing_ok=True)
            else: atomic_write(path,data,mode)
        systemctl('daemon-reload',check=False)
        systemctl('restart','mopidy.service',check=False)
        if was_active: systemctl('start','mopytui-mpd-proxy.service',check=False)
        print('Установка не завершена; прежние файлы восстановлены.',file=sys.stderr)
        raise
    print('Готово: обычный mpc — порт 6600, Mopidy — порт 6602.\nГорячие клавиши менять не нужно.\nЗапуск радио из этой папки: ./start-radio-proxy.sh')


if __name__=='__main__':
    try: main()
    except Exception as error:
        print(f'Ошибка: {error}',file=sys.stderr)
        if isinstance(error,subprocess.CalledProcessError): print(error.stderr,file=sys.stderr)
        sys.exit(1)
