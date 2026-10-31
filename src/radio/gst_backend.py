"""Private JSON-lines audio backend, embedded in the Rust binary.

With MOPYTUI_RADIO_PROXY, curl supplies a non-seekable live byte stream to
GStreamer. Network retries belong exclusively to the Rust radio-loop actor.
"""
import json
import os
import select
import subprocess
import sys
import time
import tempfile
import gi

gi.require_version('Gst', '1.0')
from gi.repository import Gst
Gst.init(None)


def emit(kind, **kw):
    print(json.dumps(dict(event=kind, **kw)), flush=True)


def stop_transport(process):
    if process is not None:
        if process.poll() is None:
            process.kill()
        process.wait()
        process.stdout.close()
        process.stderr.close()


def configure_live_source(_playbin, source):
    # No HTTP byte-range recovery: let our actor reopen the URL from the live edge.
    if source.find_property('iradio-mode'):
        source.set_property('iradio-mode', True)
    if source.find_property('retries'):
        source.set_property('retries', 0)
    if source.find_property('is-live'):
        source.set_property('is-live', False)


def make_pipeline(uri, probe, handoff):
    transport = None
    try:
        seconds = 0.0 if probe else max(0., min(30., float(os.environ.get('MOPYTUI_BUFFER_SECONDS', '0'))))
        buffering = (f'queue2 name=reservoir use-buffering=true max-size-bytes=0 '
                     f'max-size-buffers=0 max-size-time={int(seconds * Gst.SECOND)} '
                     'low-watermark=0.1 high-watermark=0.95 ! ') if seconds else ''
        sink = Gst.parse_bin_from_description(
            buffering + 'audioconvert ! audioresample ! identity name=decoded signal-handoffs=true '
            '! volume name=gain volume=0 ! ' +
            ('fakesink sync=false' if probe else 'autoaudiosink'), True)
        sink.get_by_name('decoded').connect('handoff', handoff)
        proxy = os.environ.get('MOPYTUI_RADIO_PROXY', '')
        if proxy and uri.lower().startswith(('http://', 'https://')):
            # curl honors SOCKS remote DNS, follows redirects, asks for raw audio
            # with ICY metadata, and never issues Range / resume.
            headers = tempfile.TemporaryFile()
            transport = subprocess.Popen([
                'curl', '-q', '--fail', '--silent', '--show-error', '--location', '--no-buffer',
                '--proto', '=http,https', '--proto-redir', '=http,https',
                '--connect-timeout', '5', '--speed-time', str(max(15, int(seconds * 3))), '--speed-limit', '1',
                '--retry', '0', '--header', 'Icy-MetaData: 1',
                '--dump-header', f'/dev/fd/{headers.fileno()}',
                '--user-agent', 'mopytui-radio/0.1.1',
                '--proxy', proxy, '--noproxy', '', '--url', uri,
            ], stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, pass_fds=(headers.fileno(),))
            try:
                # stdout becomes readable only after the final response headers.
                if not select.select([transport.stdout], [], [], 5)[0]:
                    raise RuntimeError('No stream response headers within 5 seconds')
                raw_headers = os.pread(headers.fileno(), 65536, 0).decode('latin-1')
                blocks = raw_headers.replace('\r\n', '\n').strip().split('\n\n')
                values = {}
                for line in blocks[-1].splitlines():
                    if ':' in line:
                        key, value = line.split(':', 1); values[key.lower()] = value.strip()
                interval = int(values.get('icy-metaint', '0'))
                if not 0 <= interval <= 2147483647:
                    raise RuntimeError('Invalid ICY metadata interval')
            finally:
                headers.close()
            os.set_blocking(transport.stderr.fileno(), False)
            pipeline = Gst.Pipeline.new(None)
            source = Gst.ElementFactory.make('fdsrc', None)
            decoder = Gst.ElementFactory.make('decodebin', None)
            if not source or not decoder:
                raise RuntimeError('GStreamer fdsrc/decodebin plugin is missing')
            source.set_property('fd', transport.stdout.fileno())
            if source.find_property('is-live'):
                source.set_property('is-live', False)
            for element in (source, decoder, sink):
                pipeline.add(element)
            if interval:
                caps = Gst.ElementFactory.make('capsfilter', None)
                caps.set_property('caps', Gst.Caps.from_string(f'application/x-icy,metadata-interval=(int){interval}'))
                pipeline.add(caps)
                linked = source.link(caps) and caps.link(decoder)
            else:
                linked = source.link(decoder)
            if not linked:
                raise RuntimeError('Cannot link live stream decoder')

            def link_audio(_decoder, pad):
                caps = pad.get_current_caps() or pad.query_caps(None)
                target = sink.get_static_pad('sink')
                if caps and caps.get_size() and caps.get_structure(0).get_name().startswith('audio/x-raw') and not target.is_linked():
                    pad.link(target)

            decoder.connect('pad-added', link_audio)
        else:
            pipeline = Gst.ElementFactory.make('playbin', None)
            if pipeline is None:
                raise RuntimeError('GStreamer playbin plugin is missing')
            pipeline.connect('source-setup', configure_live_source)
            pipeline.set_property('uri', uri)
            pipeline.set_property('audio-sink', sink)
            pipeline.set_property('video-sink', Gst.ElementFactory.make('fakesink', None))
            pipeline.set_property('flags', 2)  # audio only, no video windows
        return pipeline, sink.get_by_name('gain'), transport
    except BaseException:
        stop_transport(transport)
        raise


def play_signal(mode):
    frequency = 660 if mode == 'cue-on' else 440
    level = max(0., min(0.12, float(os.environ.get('MOPYTUI_CUE_VOLUME', '0.08'))))
    pipeline = Gst.parse_launch(f'audiotestsrc wave=sine freq={frequency} ! audioconvert ! audioresample ! volume name=tone volume=0 ! ' + 'autoaudiosink')
    try:
        pipeline.set_state(Gst.State.PLAYING)
        pipeline.get_state(Gst.SECOND)
        volume = pipeline.get_by_name('tone')
        for i in range(18):
            volume.set_property('volume', level * min(i / 3., (17 - i) / 5., 1.))
            time.sleep(.01)
    finally:
        pipeline.set_state(Gst.State.NULL)


def main():
    if os.name == 'nt':
        raise RuntimeError('Radio audio currently requires a POSIX system (Linux/macOS)')
    uri, mode = sys.argv[1:3]
    if mode in ('cue-on', 'cue-off'):
        play_signal(mode)
        return
    probe = mode == 'probe'
    last_audio = [0.0]

    def handoff(_element, buffer):
        if buffer.get_size() > 0:
            last_audio[0] = time.monotonic()

    pipeline, gain, transport = make_pipeline(uri, probe, handoff)
    bus = pipeline.get_bus()
    pending = b''
    last_sent = 0.0
    buffering = not probe and float(os.environ.get("MOPYTUI_BUFFER_SECONDS", "0")) > 0
    desired_volume = 0.0
    paused = False
    transport_error = b''
    stderr_open = transport is not None
    metadata = {"artist": "", "title": "", "album": ""}
    try:
        if buffering: emit('buffering', percent=0)
        initial = pipeline.set_state(Gst.State.PAUSED if buffering else Gst.State.PLAYING)
        if initial == Gst.StateChangeReturn.FAILURE:
            raise RuntimeError('GStreamer could not start playback')
        live_source = initial == Gst.StateChangeReturn.NO_PREROLL
        if live_source:
            # True live sources cannot fill while paused. Keep their native
            # buffering instead of deadlocking the added pre-roll queue.
            reservoir = pipeline.get_by_name('reservoir')
            if reservoir: reservoir.set_property('use-buffering', False)
            buffering = False
            emit('buffering', percent=100)
            pipeline.set_state(Gst.State.PLAYING)
        while True:
            if probe and last_audio[0] > 0:
                emit('audio')
                return
            # Drain curl's stderr before reporting the pipeline's secondary EOS.
            if stderr_open:
                try:
                    chunk = os.read(transport.stderr.fileno(), 4096)
                    if chunk:
                        transport_error = (transport_error + chunk)[-4096:]
                    else:
                        stderr_open = False
                except BlockingIOError:
                    pass
            # An exited curl can still have playable audio in the reservoir.
            # Drain that audio before reporting the terminal EOS.
            while True:
                msg = bus.pop_filtered(Gst.MessageType.ERROR | Gst.MessageType.EOS | Gst.MessageType.TAG | Gst.MessageType.BUFFERING)
                if msg is None: break
                if msg.type == Gst.MessageType.BUFFERING:
                    percent = msg.parse_buffering()
                    if live_source: continue
                    if buffering != (percent < 100):
                        pipeline.set_state(Gst.State.PAUSED if percent < 100 else Gst.State.PLAYING)
                    buffering = percent < 100
                    emit('buffering', percent=percent)
                    msg = None
                if msg and msg.type == Gst.MessageType.TAG:
                    tags = msg.parse_tag()
                    changed = {}
                    for key in ('artist', 'title', 'album'):
                        ok, value = tags.get_string(key)
                        if ok: changed[key] = value or ''
                    if 'title' in changed:
                        # ICY commonly sends a single 'Artist - Title' string.
                        if 'artist' not in changed:
                            changed['artist'] = ''
                            if ' - ' in changed['title']:
                                changed['artist'], changed['title'] = changed['title'].split(' - ', 1)
                        changed.setdefault('album', '')
                    if changed:
                        metadata.update(changed)
                        emit('metadata', **metadata)
                    msg = None
                if msg:
                    if msg.type == Gst.MessageType.ERROR:
                        err, debug = msg.parse_error()
                        emit('error', message=str(err))
                        print(debug, file=sys.stderr, flush=True)
                    else:
                        emit('error', message=transport_error.decode(errors='replace').strip() or 'End of stream')
                    return
            now = time.monotonic()
            if not buffering and last_audio[0] > last_sent and now - last_sent >= 0.2:
                emit('audio')
                last_sent = now
            readable, _, _ = select.select([sys.stdin.buffer], [], [], 0.025)
            if readable:
                chunk = os.read(sys.stdin.fileno(), 4096)
                if not chunk:
                    return
                pending += chunk
                while b'\n' in pending:
                    line, pending = pending.split(b'\n', 1)
                    command = json.loads(line)
                    if command.get('stop'):
                        return
                    if 'volume' in command:
                        desired_volume = max(0., min(1., command['volume']))
                    if 'paused' in command:
                        paused = bool(command['paused'])
                    # Live pause mutes output while draining the stream, so resume
                    # returns to live audio without a seek or stale buffered data.
                    gain.set_property('volume', 0.0 if paused else desired_volume)
    finally:
        stop_transport(transport)
        pipeline.set_state(Gst.State.NULL)


if __name__ == '__main__':
    try:
        main()
    except Exception as exc:
        emit('error', message=str(exc))
        sys.exit(1)
