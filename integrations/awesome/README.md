# Radio metadata for the supplied lain.widget.mpd

This is a modified copy of the Lua file supplied by the user, under GPL v2.
The Rust TUI remains under its existing license. COPYING contains GPL v2.

When Radio Loop is active the MPD gateway returns:

- `Name`: the selected radio station;
- `Artist`, `Title`, `Album`: metadata sent by the stream;
- `Comment: mopytui-radio`: explicit identification of the radio mode.

The widget displays `Station · Artist — Title`. Missing fields are omitted;
without track metadata it shows only the station. Values are Pango-escaped.
For ordinary Mopidy the existing `settings()` callback retains control of the
layout. In radio mode the combined station/artist/title string is applied after
that callback. The callback also has `mpd_now.radio`, `mpd_now.station` and
`mpd_now.display` available. Polls do not overlap, and telnet requests explicitly
bypass HTTP/SOCKS proxy environment variables. A station or artist/title change
updates the notification, including when two stations have identical titles.

## Install

1. Close the old mopytui and install v9 using `sh install-system.sh` from the
   extracted release directory. Existing proxy configuration and MPD service
   stay in place; the existing gateway is compatible with these metadata fields.
2. From the same directory run:

   ```sh
   python3 install-awesome-widget.py /full/path/to/your/lain/widget/mpd.lua
   ```

   Supply the actual existing widget path. The installer preserves a timestamped
   backup beside it and checks Lua syntax when `luac` is available. Run as the
   owner of the file; a system-owned location may require sudo.
3. Restart Awesome to reload Lua, then start Radio Loop. Existing media keys and
   the MPD port 6600 do not change.

Sources without track metadata cannot supply artist/title. ICY commonly sends
one `Artist - Title` string; the player separates it on the first ` - `.
HLS/DASH via the explicit curl proxy transport remains unsupported.

The ICY caps and demux behavior follow the official GStreamer documentation:
https://gstreamer.freedesktop.org/documentation/icydemux/index.html

## Widget-only correction: theme and automatic refresh

Radio markup contains foreground colors only. The textbox font is `args.font`
when explicitly supplied, otherwise `beautiful.font` from the loaded theme.
Station and artist use `beautiful.color_accent_2`; title and separators use
`beautiful.color_fg_base`. Each falls back to `fg_normal` when absent.
The station display is cut to the first 32 UTF-8 code points before markup
escaping; the full station remains in `mpd_now.station`. No hard-coded font,
size or bold weight is added. Ordinary playback still uses the settings callback.

Every widget instance owns an autostarting gears.timer, with an immediate first
request and a default interval of 0.5 seconds. There is no shared Lain timer name.
An in-flight request prevents overlapping polls; curl has a 3-second deadline.
This correction requires replacing only the Lua module and restarting Awesome.
The TUI, gateway and proxy configuration do not need reinstallation.

Checks: Lua 5.1 parser; isolated layout/color/UTF-8/automatic-update tests; real
gears.timer running a GLib event loop with three automatic polls and a track
change, while the MPD response and Awesome widgets are test doubles.
An external theme callback that explicitly stops `mpd.timer` can still stop it;
if that exists in the user's theme it must be examined separately.

Official timer and textbox APIs:
https://awesomewm.org/apidoc/core_components/gears.timer.html
https://awesomewm.org/apidoc/widgets/wibox.widget.textbox.html

## Second widget correction: distinct artist color and prompt polling

This supersedes the transport/color details above. Station uses color_accent_2;
artist now uses color_accent_1; track uses color_fg_base. Font and 32-character
station truncation are unchanged.

The MPD request now uses Python 3's socket library directly, reads each reply
through its OK terminator, and closes promptly. No shell/telnet proxy variables
or curl completion delay are involved. A failed poll retains the last display.
The poll interval is args.poll_interval (default 0.5 seconds). The old
args.timeout is intentionally no longer used for polling, so a pre-existing
large value cannot silently slow down radio updates.

Tests cover distinct colors, a legacy timeout=60 still polling at 0.5 seconds,
actual gears.timer/GLib automatic updates, and two real socket requests to a
fake MPD server with a changed title (29–34 ms in the local test). Actual stream
metadata arrival depends on what the station transmits; this code only makes
already available MPD metadata visible promptly.

## v10: supplied theme integration

The supplied theme split Title on a hyphen and unconditionally called :sub()
on the result. Already separated Artist/Title tags (or titles without a hyphen)
therefore caused errors inside settings(), before the widget could redraw.
The MPD section is replaced with a nil-safe renderer using the fields directly.
Artist now uses color_accent_3. Station/color_accent_2 and title/color_fg_base,
the exact theme.font, and the 32-code-point station limit are preserved.

The theme owns text and icon together via theme_handles_display=true. Playing
shows metadata; pause shows Пауза; stop shows Выключено; failed polls show
Нет соединения. Old track text is not retained in those states. Icon clicks
refresh after the mpc command has finished, not in parallel with it.

Install from the release folder:

    sh install-system.sh
    python3 install-awesome-integration.py /actual/path/to/lain/widget/mpd.lua

The integration installer defaults to ~/.config/awesome/themes/sigil/theme.lua,
as indicated by the supplied theme. Override with --theme /actual/theme.lua.
It backs up both files and replaces only the section from -- MPD to -- MEM;
all other theme contents remain exactly unchanged. Restart Awesome afterwards.
No changes to the existing proxy or gateway service are required.
