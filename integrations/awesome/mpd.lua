--[[

     Licensed under GNU General Public License v2
      * (c) 2013, Luke Bonham
      * (c) 2010, Adrian C. <anrxc@sysphere.org>

--]]

local helpers      = require("lain.helpers")
local shell        = require("awful.util").shell
local escape_f     = require("awful.util").escape
local focused      = require("awful.screen").focused
local naughty      = require("naughty")
local wibox        = require("wibox")
local theme        = require("beautiful")
local timer        = require("gears.timer")
local os           = { getenv = os.getenv }
local string       = { format = string.format,
                       gmatch = string.gmatch,
                       match  = string.match }

-- MPD infos
-- lain.widget.mpd

-- Count UTF-8 code points before escaping Pango markup (Lua 5.1 compatible).
local function station_prefix(value)
    local out, count = "", 0
    for character in value:gmatch("[%z\1-\127\194-\244][\128-\191]*") do
        if count == 32 then break end
        out, count = out .. character, count + 1
    end
    return out
end

local function colored(value, color)
    if not color then return value end
    return '<span foreground="' .. escape_f(color):gsub('"', '&quot;') .. '">' .. value .. '</span>'
end

-- Read complete MPD responses directly; no curl/telnet shutdown delay.
local mpd_query = [=[
import socket, sys
host, port, password = sys.argv[1:4]
try:
    if host.startswith('/'):
        connection = socket.socket(socket.AF_UNIX)
        connection.settimeout(0.8)
        connection.connect(host)
    else:
        connection = socket.create_connection((host, int(port)), timeout=0.8)
    with connection:
        connection.settimeout(0.8)
        with connection.makefile('rb') as reader:
            if not reader.readline(8192).startswith(b'OK MPD '):
                raise RuntimeError('Invalid MPD greeting')
            commands = []
            if password:
                if '\n' in password or '\r' in password:
                    raise RuntimeError('Invalid MPD password')
                commands.append('password "' + password.replace('\\', '\\\\').replace('"', '\\"') + '"')
            commands += ['status', 'currentsong']
            output = []
            for command in commands:
                connection.sendall((command + '\n').encode())
                for _ in range(4096):
                    line = reader.readline(8192)
                    if not line: raise RuntimeError('MPD disconnected')
                    if line.startswith(b'ACK '): raise RuntimeError(line.decode(errors='replace'))
                    if line.rstrip() == b'OK': break
                    if command in ('status', 'currentsong'):
                        output.append(line.decode('utf-8', errors='replace'))
                else: raise RuntimeError('MPD reply too large')
            sys.stdout.write(''.join(output))
except Exception as error:
    print(error, file=sys.stderr)
    sys.exit(1)
]=]

local function factory(args)
    local mpd           = { widget = wibox.widget.textbox() }
    local args          = args or {}
    local timeout       = args.poll_interval or 0.5
    local password      = args.password or ""
    local host          = args.host or os.getenv("MPD_HOST") or "127.0.0.1"
    local port          = args.port or os.getenv("MPD_PORT") or "6600"
    local music_dir     = args.music_dir or os.getenv("HOME") .. "/Music"
    local cover_pattern = args.cover_pattern or "*\\.(jpg|jpeg|png|gif)$"
    local cover_size    = args.cover_size or 100
    local default_art   = args.default_art
    local notify        = args.notify or "on"
    local followtag     = args.followtag or false
    local settings      = args.settings or function() end

    local cmd = { "python3", "-c", mpd_query, host, tostring(port), password }

    mpd_notification_preset = { title = "Now playing", timeout = 0.001 }

    helpers.set_map("current mpd track", nil)

    local updating = false
    function mpd.update()
        if updating then return end
        updating = true
        helpers.async(cmd, function(f, exit_code)
            updating = false
            if (exit_code and exit_code ~= 0) or not f:match("state: ") then
                f = "state: offline\n"
            end
            local station_raw = "Radio"
            mpd_now = {
                radio        = false,
                station      = "",
                random_mode  = false,
                single_mode  = false,
                repeat_mode  = false,
                consume_mode = false,
                pls_pos      = "N/A",
                pls_len      = "N/A",
                state        = "N/A",
                volume       = "N/A",
                file         = "N/A",
                name         = "N/A",
                artist       = "N/A",
                title        = "N/A",
                album        = "N/A",
                genre        = "N/A",
                track        = "N/A",
                date         = "N/A",
                time         = "N/A",
                elapsed      = "N/A"
            }

            for line in string.gmatch(f, "[^\n]+") do
                for k, v in string.gmatch(line, "([%w]+):[%s](.*)$") do
                    v = v:gsub("\r$", "")
                    if     k == "state"          then mpd_now.state        = v
                    elseif k == "file"           then mpd_now.file         = v
                    elseif k == "Name"           then mpd_now.name = escape_f(v); station_raw = v
                    elseif k == "Comment"        then mpd_now.radio        = v == "mopytui-radio"
                    elseif k == "Artist"         then mpd_now.artist       = escape_f(v)
                    elseif k == "Title"          then mpd_now.title        = escape_f(v)
                    elseif k == "Album"          then mpd_now.album        = escape_f(v)
                    elseif k == "Genre"          then mpd_now.genre        = escape_f(v)
                    elseif k == "Track"          then mpd_now.track        = escape_f(v)
                    elseif k == "Date"           then mpd_now.date         = escape_f(v)
                    elseif k == "Time"           then mpd_now.time         = v
                    elseif k == "elapsed"        then mpd_now.elapsed      = string.match(v, "%d+")
                    elseif k == "song"           then mpd_now.pls_pos      = v
                    elseif k == "volume"         then mpd_now.volume      = v
                    elseif k == "playlistlength" then mpd_now.pls_len      = v
                    elseif k == "repeat"         then mpd_now.repeat_mode  = v ~= "0"
                    elseif k == "single"         then mpd_now.single_mode  = v ~= "0"
                    elseif k == "random"         then mpd_now.random_mode  = v ~= "0"
                    elseif k == "consume"        then mpd_now.consume_mode = v ~= "0"
                    end
                end
            end

            mpd_notification_preset.text = string.format("%s (%s) - %s\n%s", mpd_now.artist,
                                           mpd_now.album, mpd_now.date, mpd_now.title)
            local radio_text = nil
            if mpd_now.radio then
                mpd_now.station = mpd_now.name ~= "N/A" and mpd_now.name or "Radio"
                local accent = theme.color_accent_2 or theme.fg_normal
                local foreground = theme.color_fg_base or theme.fg_normal
                local artist_accent = theme.color_accent_3 or theme.fg_normal
                radio_text = colored(escape_f(station_prefix(station_raw)), accent)
                local artist = mpd_now.artist ~= "N/A" and mpd_now.artist or ""
                local title = mpd_now.title ~= "N/A" and mpd_now.title or ""
                if #artist > 0 then radio_text = radio_text .. colored(" · ", foreground) .. colored(artist, artist_accent) end
                if #title > 0 and title ~= mpd_now.station then radio_text = radio_text .. colored(" — " .. title, foreground) end
                mpd_now.display = radio_text
                mpd_notification_preset.text = radio_text
            end
            widget = mpd.widget
            settings()
            -- Keep the user's existing settings callback for ordinary Mopidy.
            -- Radio fields are already Pango-escaped by the parser above.
            if not args.theme_handles_display and mpd_now.state ~= "play" then
                local state_text = mpd_now.state == "pause" and "Пауза" or (mpd_now.state == "offline" and "Нет соединения" or "Выключено")
                mpd.widget.font = args.font or theme.font
                mpd.widget:set_markup(colored(state_text, theme.color_fg_base or theme.fg_normal))
            elseif radio_text and not args.theme_handles_display then
                -- No hard-coded font, weight or size in the markup.
                mpd.widget.font = args.font or theme.font
                mpd.widget:set_markup(radio_text)
            end
            local track_key = mpd_now.name .. "|" .. mpd_now.artist .. "|" .. mpd_now.title

            if mpd_now.state == "play" then
                if notify == "on" and track_key ~= helpers.get_map("current mpd track") then
                    helpers.set_map("current mpd track", track_key)

                    if followtag then mpd_notification_preset.screen = focused() end

                    local common =  {
                        preset      = mpd_notification_preset,
                        icon        = default_art,
                        icon_size   = cover_size,
                        replaces_id = mpd.id
                    }

                    if not string.match(mpd_now.file, "http.*://") then -- local file instead of http stream
                        local path   = string.format("%s/%s", music_dir, string.match(mpd_now.file, ".*/"))
                        local cover  = string.format("find '%s' -maxdepth 1 -type f | egrep -i -m1 '%s'",
                                       path:gsub("'", "'\\''"), cover_pattern)
                        helpers.async({ shell, "-c", cover }, function(current_icon)
                            common.icon = current_icon:gsub("\n", "")
                            if #common.icon == 0 then common.icon = nil end
                            mpd.id = naughty.notify(common).id
                        end)
                    else
                        mpd.id = naughty.notify(common).id
                    end

                end
            elseif mpd_now.state ~= "pause" then
                helpers.set_map("current mpd track", nil)
            end
        end)
    end

    -- An independent repeating timer, not Lain's shared timer named "mpd".
    mpd.timer = timer { timeout = timeout, autostart = true,
                        call_now = true, callback = mpd.update }

    return mpd
end

return factory
