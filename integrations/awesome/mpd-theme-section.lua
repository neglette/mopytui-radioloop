-- MPD: one renderer owns the text and icon for both ordinary music and radio.
local mpdicon = wibox.widget.imagebox(theme.widget_headphone)
local function mpd_control(command)
    awful.spawn.easy_async({ "mpc", command }, function()
        if theme.mpd then theme.mpd.update() end
    end)
end
mpdicon:buttons(awful.util.table.join(
    awful.button({}, 1, function() mpd_control("toggle") end),
    awful.button({}, 3, function() mpd_control("stop") end)))

local function mpd_present(value)
    return value and value ~= "N/A" and value ~= "" and value or nil
end
local function mpd_station_prefix(value)
    -- Name was Pango-escaped by the widget; decode before counting characters.
    value = value:gsub("&lt;", "<"):gsub("&gt;", ">"):gsub("&quot;", '"'):gsub("&apos;", "'"):gsub("&amp;", "&")
    local result, count = "", 0
    for character in value:gmatch("[%z\1-\127\194-\244][\128-\191]*") do
        if count == 32 then break end
        result, count = result .. character, count + 1
    end
    return awful.util.escape(result)
end
local function mpd_clock(value)
    local seconds = tonumber(value)
    if not seconds then return nil end
    return string.format("%d:%02d", math.floor(seconds / 60), math.floor(seconds % 60))
end

theme.mpd = lain.widget.mpd({
    poll_interval = 0.5,
    theme_handles_display = true,
    settings = function()
        local info = mpd_now
        local state = info.state
        local text
        if state == "play" then
            local station = mpd_present(info.name)
            local artist = mpd_present(info.artist)
            local title = mpd_present(info.title)
            local is_stream = info.radio or (info.file or ""):match("^https?://")
            mpdicon:set_image(is_stream and theme.widget_music_net or theme.widget_music_on)
            text = ""
            if is_stream and station then
                text = markup(theme.color_accent_2, mpd_station_prefix(station))
            end
            if artist then
                if text ~= "" then text = text .. markup(theme.color_fg_base, " · ") end
                text = text .. markup(theme.color_accent_3, artist)
            end
            if title and (not is_stream or title ~= station) then
                if text ~= "" then text = text .. markup(theme.color_fg_base, " — ") end
                text = text .. markup(theme.color_fg_base, title)
            end
            if text == "" then text = markup(theme.color_fg_base, "Воспроизведение") end
            if not is_stream then
                local elapsed, total = mpd_clock(info.elapsed), mpd_clock(info.time)
                if elapsed and total then text = text .. markup(theme.color_fg_base, " [" .. elapsed .. "/" .. total .. "]") end
            end
        elseif state == "pause" then
            mpdicon:set_image(theme.widget_music_pause)
            text = markup(theme.color_fg_base, "Пауза")
        elseif state == "offline" or state == "N/A" then
            mpdicon:set_image(theme.widget_music_stop)
            text = markup(theme.color_fg_base, "Нет соединения")
        else
            mpdicon:set_image(theme.widget_music_stop)
            text = markup(theme.color_fg_base, "Выключено")
        end
        widget:set_markup(markup.font(theme.font, thinspace .. text .. thinspace))
    end
})

