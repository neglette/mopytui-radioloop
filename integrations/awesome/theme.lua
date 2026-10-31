--[[

     Powerarrow Awgesome WM theme
     github.com/lcpz

--]]

local gears             = require("gears")
local lain              = require("lain")
local awful             = require("awful")
local beautiful         = require("beautiful")
local wibox             = require("wibox")
local caldav_tasks = require("widgets.caldav-tasks")
local os, math, string  = os, math, string


local theme                                     = {}
theme.dir                                       = os.getenv("HOME") .. "/.config/awesome/themes/sigil"

theme.wall_center                               = "/home/user/.config/awesome/wallpapers/sweet.jpeg"
theme.wall_right                                = "/home/user/.config/awesome/wallpapers/sign.png"
theme.wall_left                                 = "/home/user/.config/awesome/wallpapers/sign.png"
theme.wall_touch                                = "/home/user/.config/awesome/wallpapers/sign.png"
theme.wall_other                                = "/home/user/.config/awesome/default.jpg"

theme.font                                      = "0xProto Nerd Font Mono 12"
theme.font_big                                  = "0xProto Nerd Font Mono 13"
theme.taglist_font                              = "0xProto Nerd Font Mono 12"
theme.border_width                              = 0.5

theme.color_bg_base                             = "#000000"
theme.color_bg_strip                            = "#3F3F3F"
theme.color_fg_base                             = "#ffffff"
theme.color_fg_bright                           = "#EEEEEE"
theme.color_urgent                              = "#FFE837"
theme.color_accent_3                            = "#00ccff"
theme.color_accent_2                            = "#Ff4400"
theme.color_accent_1                            = "#FF00ff"

-- Цвета обычных тегов по типам. Эти цвета применяются только когда тег не выбран
-- и не находится в urgent-состоянии; focus/urgent остаются штатными.
theme.tag_type_colors = {
    text      = { bg = "#3A3A3A", fg = "#F2F2F2" },

    mail      = { bg = "#68407A", fg = "#F8E9FF" },
    code      = { bg = "#1C6645", fg = "#E1FFEF" },
    message   = { bg = "#2D5682", fg = "#E4F0FF" },
    file      = { bg = "#7A5723", fg = "#FFF0CF" },
    browser   = { bg = "#176A78", fg = "#E2FBFF" },
    reading   = { bg = "#5B4580", fg = "#F3E9FF" },
    torrent   = { bg = "#8A4B21", fg = "#FFF0E2" },
    game      = { bg = "#7F4156", fg = "#FFE8EF" },
    media     = { bg = "#3D4F8C", fg = "#E8EDFF" },
    office    = { bg = "#6A6829", fg = "#FFFDCB" },
    cad       = { bg = "#1E6A5B", fg = "#DFFFF2" },
    graphics  = { bg = "#85407C", fg = "#FFE8FB" },
    reference = { bg = "#335F84", fg = "#E8F5FF" },
    knowledge = { bg = "#56418A", fg = "#EEE8FF" },
    ai = { bg = "#af31aA", fg = "#EEE8FF" },


    virtual   = { bg = "#514B5C", fg = "#F3EEFA" },
    system    = { bg = "#46515C", fg = "#EEF3F7" },
    tool      = { bg = "#66503A", fg = "#FFF0DC" },

    planning  = { bg = "#4B6C31", fg = "#EFFFD9" },
    network   = { bg = "#1E6F73", fg = "#DFFFFF" },
    tablet    = { bg = "#6B4A85", fg = "#F6E8FF" },
    home      = { bg = "#3E6E40", fg = "#E9FFE9" },
    time      = { bg = "#38548A", fg = "#E8EEFF" },
    audio     = { bg = "#7A5A2F", fg = "#FFF2DF" },
    calc      = { bg = "#2E6674", fg = "#E1FBFF" },

    pip       = { bg = "#5A5A5A", fg = theme.color_fg_bright },
}
theme.tag_type_default = { bg = theme.color_bg_base, fg = theme.color_fg_base }

theme.fg_normal                                 = theme.color_fg_base
theme.bg_normal                                 = theme.color_bg_base
theme.fg_focus                                  = theme.color_bg_base
theme.bg_focus                                  = theme.color_fg_bright
theme.fg_urgent                                 = theme.color_bg_base
theme.bg_urgent                                 = theme.color_urgent
theme.taglist_fg_focus                          = theme.color_bg_base
theme.tasklist_fg_focus                         = theme.color_accent_1
theme.tasklist_bg_focus                         = theme.color_bg_base
theme.border_normal                             = theme.color_bg_base
theme.border_focus                              = theme.color_bg_base
theme.border_marked                             = theme.color_accent_2
theme.titlebar_bg_focus                         = theme.color_bg_strip
theme.titlebar_bg_normal                        = theme.color_bg_strip
theme.titlebar_bg_focus                         = theme.bg_focus
theme.titlebar_bg_normal                        = theme.bg_normal
theme.titlebar_fg_focus                         = theme.fg_focus

-- CalDAV task widget. These values must be assigned only after bg_normal and
-- fg_normal have been initialized above.
theme.caldav_tasks_bg                           = theme.bg_normal
theme.caldav_tasks_fg                           = theme.fg_normal
theme.caldav_tasks_muted_fg                     = "#7f849c"
theme.caldav_tasks_overdue_fg                   = "#800020"
theme.caldav_tasks_checkbox_color               = "#89b4fa"
theme.caldav_tasks_priority_high_color          = "#f38ba8"
theme.caldav_tasks_priority_normal_color        = "#f9e2af"
theme.caldav_tasks_priority_low_color           = "#7f849c"
theme.caldav_tasks_icon_font                    = "0xProto Nerd Font Mono 16"

theme.icon_theme                                = "Papirus"
theme.menu_height                               = 16
theme.menu_width                                = 140
theme.menu_submenu_icon                         = theme.dir .. "/icons/submenu.png"
theme.awesome_icon1                             = theme.dir .. "/icons/awesome_icon.png"
theme.taglist_squares_sel                       = theme.dir .. "/icons/square_sel.png"
theme.taglist_squares_unsel                     = theme.dir .. "/icons/square_unsel.png"
theme.layout_tile                               = theme.dir .. "/icons/tile.png"
theme.layout_tileleft                           = theme.dir .. "/icons/tileleft.png"
theme.layout_tilebottom                         = theme.dir .. "/icons/tilebottom.png"
theme.layout_tiletop                            = theme.dir .. "/icons/tiletop.png"
theme.layout_fairv                              = theme.dir .. "/icons/fairv.png"
theme.layout_fairh                              = theme.dir .. "/icons/fairh.png"
theme.layout_spiral                             = theme.dir .. "/icons/spiral.png"
theme.layout_dwindle                            = theme.dir .. "/icons/dwindle.png"
theme.layout_max                                = theme.dir .. "/icons/max.png"
theme.layout_fullscreen                         = theme.dir .. "/icons/fullscreen.png"
theme.layout_floating                           = theme.dir .. "/icons/floating.png"
theme.layout_centerwork                         = theme.dir .. "/icons/centerwork.png"
theme.layout_centerworkh                        = theme.dir .. "/icons/centerworkh.png"
theme.layout_termfair                           = theme.dir .. "/icons/termfair.png"
theme.layout_centerfair                         = theme.dir .. "/icons/centerfair.png"
theme.layout_magnifier                          = theme.dir .. "/icons/magnifier.png"
theme.widget_ac                                 = theme.dir .. "/icons/ac.png"
theme.widget_battery                            = theme.dir .. "/icons/battery.png"
theme.widget_battery_low                        = theme.dir .. "/icons/battery_low.png"
theme.widget_battery_empty                      = theme.dir .. "/icons/battery_empty.png"
theme.widget_mem                                = theme.dir .. "/icons/mem.png"
theme.widget_cpu                                = theme.dir .. "/icons/cpu.png"
theme.widget_temp                               = theme.dir .. "/icons/temp.png"
theme.widget_net                                = theme.dir .. "/icons/net.png"
theme.widget_hdd                                = theme.dir .. "/icons/hdd.png"
theme.widget_music                              = theme.dir .. "/icons/note.png"
theme.widget_music_on                           = theme.dir .. "/icons/note_on.png"
theme.widget_music_net                          = theme.dir .. "/icons/music_net.png"
theme.widget_music_pause                        = theme.dir .. "/icons/pause.png"
theme.widget_music_stop                         = theme.dir .. "/icons/stop.png"
theme.widget_volume                             = theme.dir .. "/icons/volume.png"
theme.widget_vol                                = theme.dir .. "/icons/vol.png"
theme.widget_vol_low                            = theme.dir .. "/icons/vol_low.png"
theme.widget_vol_no                             = theme.dir .. "/icons/vol_no.png"
theme.widget_vol_mute                           = theme.dir .. "/icons/vol_mute.png"
theme.widget_mail                               = theme.dir .. "/icons/mail_on.png"
theme.widget_task                               = theme.dir .. "/icons/task.png"
theme.widget_scissors                           = theme.dir .. "/icons/scissors.png"
theme.widget_calc                               = theme.dir .. "/icons/calc.png"
theme.widget_alarm                              = theme.dir .. "/icons/alarm.png"
theme.widget_color                              = theme.dir .. "/icons/color.png"
theme.widget_headphone                          = theme.dir .. "/icons/phones.png"
theme.tasklist_plain_task_name                  = false
theme.tasklist_disable_icon                     = true
theme.useless_gap                               = 0
theme.titlebar_close_button_focus               = theme.dir .. "/icons/titlebar/close_focus.png"
theme.titlebar_close_button_normal              = theme.dir .. "/icons/titlebar/close_normal.png"
theme.titlebar_ontop_button_focus_active        = theme.dir .. "/icons/titlebar/ontop_focus_active.png"
theme.titlebar_ontop_button_normal_active       = theme.dir .. "/icons/titlebar/ontop_normal_active.png"
theme.titlebar_ontop_button_focus_inactive      = theme.dir .. "/icons/titlebar/ontop_focus_inactive.png"
theme.titlebar_ontop_button_normal_inactive     = theme.dir .. "/icons/titlebar/ontop_normal_inactive.png"
theme.titlebar_sticky_button_focus_active       = theme.dir .. "/icons/titlebar/sticky_focus_active.png"
theme.titlebar_sticky_button_normal_active      = theme.dir .. "/icons/titlebar/sticky_normal_active.png"
theme.titlebar_sticky_button_focus_inactive     = theme.dir .. "/icons/titlebar/sticky_focus_inactive.png"
theme.titlebar_sticky_button_normal_inactive    = theme.dir .. "/icons/titlebar/sticky_normal_inactive.png"
theme.titlebar_floating_button_focus_active     = theme.dir .. "/icons/titlebar/floating_focus_active.png"
theme.titlebar_floating_button_normal_active    = theme.dir .. "/icons/titlebar/floating_normal_active.png"
theme.titlebar_floating_button_focus_inactive   = theme.dir .. "/icons/titlebar/floating_focus_inactive.png"
theme.titlebar_floating_button_normal_inactive  = theme.dir .. "/icons/titlebar/floating_normal_inactive.png"
theme.titlebar_maximized_button_focus_active    = theme.dir .. "/icons/titlebar/maximized_focus_active.png"
theme.titlebar_maximized_button_normal_active   = theme.dir .. "/icons/titlebar/maximized_normal_active.png"
theme.titlebar_maximized_button_focus_inactive  = theme.dir .. "/icons/titlebar/maximized_focus_inactive.png"
theme.titlebar_maximized_button_normal_inactive = theme.dir .. "/icons/titlebar/maximized_normal_inactive.png"
theme.widget_clock                              = theme.dir .. "/icons/clock.png"
thinspace = " "

local markup = lain.util.markup
local separators = lain.util.separators

local fs_widget = require("awesome-wm-widgets.fs-widget.fs-widget")
local ram_widget = require("awesome-wm-widgets.ram-widget.ram-widget")
local cpu_widget = require("awesome-wm-widgets.cpu-widget.cpu-widget")
local net_speed_widget = require("awesome-wm-widgets.net-speed-widget.net-speed")

theme.kbd = {}

local keyboardlayout = awful.widget.keyboardlayout()

theme.kbd.widget = wibox.widget {
    widget = wibox.widget.textbox,
    align  = "center",
    valign = "center"
}

local caldav_tasks_widget = caldav_tasks {
    refresh_interval = 60,
    request_timeout = 25,

    max_tasks = 18,
    popup_width = 865,
    popup_screen = 2,

    font = theme.font,
    title_font = theme.font,
    due_font = theme.font,
    calendar_font = theme.font,
    icon_font = theme.caldav_tasks_icon_font,

    bg = theme.caldav_tasks_bg,
    fg = theme.caldav_tasks_fg,
    muted_fg = theme.caldav_tasks_muted_fg,
    overdue_fg = theme.caldav_tasks_overdue_fg,
    checkbox_color = theme.caldav_tasks_checkbox_color,
    priority_high_color = theme.caldav_tasks_priority_high_color,
    priority_normal_color = theme.caldav_tasks_priority_normal_color,
    priority_low_color = theme.caldav_tasks_priority_low_color,

    popup_shape = gears.shape.rectangle,
}

-- Export the instance for other parts of the Awesome configuration.
theme.caldav_tasks = caldav_tasks_widget

local function update()
    local layout = keyboardlayout.widget.text

    if not layout then return end

    layout = layout:lower()

    if layout:match("us") then
        theme.kbd.widget.markup =
            markup.font(theme.font_big,
                markup("#DDDDDD", "US"))

    elseif layout:match("ru") then
        theme.kbd.widget.markup =
            markup.font(theme.font_big,
                markup("#FF0000", "RU"))
    end
end

keyboardlayout:connect_signal("widget::redraw_needed", update)

-- небольшая отложенная инициализация
gears.timer.start_new(0.5, function()
    update()
    return false
end)


-- Textclock
os.setlocale("en_US") -- to localize the clock
local clockicon = wibox.widget.imagebox(theme.widget_clock)
-- local mytextclock = wibox.widget.textclock(markup(theme.color_fg_bright, "W%V,") .. markup(theme.color_accent_1," %a,") .. markup(theme.color_fg_bright, " %d.%m.%Y,") .. markup(theme.color_accent_1, " %H:%M"))
local mytextclock = 
wibox.widget.textclock(markup(theme.color_accent_2, "W%V ") 
.. markup(theme.color_accent_3, "%a,") 
.. markup(theme.color_fg_bright, " %b %d, ") 
.. markup(theme.color_accent_1, "%H:%M:%S")) --with time
-- local mytextclock = wibox.widget.textclock(markup(theme.color_accent_2, "W_%V") .. markup(theme.color_fg_bright, " %Y-%m-%d")) --no time
mytextclock.font = "0xProto Nerd Font Mono 12"
mytextclock.refresh = 0.5

-- ПН
-- ВТ ♂
-- СР
-- ЧТ
-- ПТ
-- СБ
-- ВС

-- Binary clock
local binclock = require("themes.powerarrow.binclock"){
    height = 16,
    show_seconds = true,
    color_active = theme.fg_normal,
    color_inactive = theme.bg_focus
}


local calendar_widget = require("awesome-wm-widgets.calendar-widget-2x2.calendar")
local cw = calendar_widget({
    theme = 'sigil',
    placement = 'top_right',
    start_sunday = false,
    radius = 5,
    previous_month_button = 5,
    next_month_button = 4,
})
mytextclock:connect_signal("button::press",
    function(_, _, _, button)
        if button == 1 then cw.toggle() end
    end)


-- -- Calendar
-- theme.cal = lain.widget.calendar({
--     --cal = "cal --color=always",
--     attach_to = { mytextclock },
--     notification_preset = {
--         font = "Fira Code Retina 10.75",
--         fg   = theme.fg_normal,
--         bg   = theme.bg_normal
--     }
-- })

-- Taskwarrior
local task = wibox.widget.imagebox(theme.widget_task)
lain.widget.contrib.task.attach(task, {
    -- do not colorize output
    show_cmd = "task | sed -r 's/\\x1B\\[([0-9]{1,2}(;[0-9]{1,2})?)?[mGK]//g'"
})
task:buttons(awful.util.table.join(awful.button({}, 3, lain.widget.contrib.task.prompt)))

-- Scissors (xsel copy and paste)
local scissors = wibox.widget.imagebox(theme.widget_scissors)
scissors:buttons(awful.util.table.join(awful.button({}, 1, function() awful.spawn("flameshot gui") end)))

local calc = wibox.widget.imagebox(theme.widget_calc)
calc:buttons(awful.util.table.join(awful.button({}, 1, function() awful.spawn("qalculate-gtk") end)))

-- local color = wibox.widget.imagebox(theme.widget_color)
-- color:buttons(awful.util.table.join(awful.button({}, 1, function() awful.spawn("bash /home/user/Scripts/color.sh") end)))

local alarm = wibox.widget.imagebox(theme.widget_alarm)
alarm:buttons(awful.util.table.join(awful.button({}, 1, function() awful.spawn("kalarm") end)))

-- Mail IMAP check
local mailicon = wibox.widget.imagebox(theme.widget_mail)
--  commented because it needs to be set before use
-- mailicon:buttons(awful.util.table.join(awful.button({ }, 1, function () awful.spawn(mail) end)))
-- local mail = lain.widget.imap({
--     timeout  = 180,
--     server   = "imap.rambler.ru",
--     settings = function()
--         if mailcount > 0 then
--             widget:set_text(" " .. mailcount .. " ")
--             mailicon:set_image(theme.widget_mail_on)
--         else
--             widget:set_text("")
--             mailicon:set_image(theme.widget_mail)
--         end
--     end
-- })
--

-- ALSA volume
theme.volume = lain.widget.alsabar({
    --togglechannel = "IEC958,3",
    notification_preset = { font = "xos4 Terminus 10", fg = theme.fg_normal },
})

-- local headphoneicon = wibox.widget.imagebox(theme.widget_headphone)

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

-- MEM
local memicon = wibox.widget.imagebox(theme.widget_mem)
local mem = lain.widget.mem({
    settings = function()
    local memory = tonumber(mem_now.used)
    local swap = tonumber(mem_now.swapused)  
    -- local memory = tonumber(mem_now.used) / 1024
    -- local swap = tonumber(mem_now.swapused) / 1024
    local memory_all = tostring((tonumber(mem_now.total)) / 1024)
    local swap_all = tostring((tonumber(mem_now.swap)) / 1024)
    if memory < 10 then memory = string.sub(100 + memory, 2) end
        widget:set_markup(
        markup.fontfg(theme.font, theme.color_fg_base, 
            string.format("%05d", memory) 
            -- ..
            -- thinspace
            -- .. 
            -- "GB/" 
            -- .. 
            -- string.format("%.4s", memory_all) 
            ..
            thinspace
            -- .. 
            -- "GB "
            )
        ..
        -- markup.fontfg(theme.font, theme.color_fg_base, 
        --     string.format("%5d", swap) 
            -- .. 
            thinspace
            -- ..
            -- "GB/" 
            -- .. 
            -- string.format("%.4s", swap_all) 
            -- .. 
            -- thinspace
            -- ..
            -- "GB "
            -- )
            )
    end
})

local vramicon = wibox.widget.imagebox(theme.widget_vram)
local vram = lain.widget.vram({
    settings = function()
        local vused = tonumber(vram_now.used) / 1024   -- ГБ
       -- local vall  = vram_now.total / 1024  -- ГБ
        widget:set_markup(
            markup.fontfg(theme.font, theme.color_accent_1,
             --   string.format("%.1f/%.1f (%d%%)", vused, vall, vram_now.perc)
                string.format("%05d", vram_now.used)
             .. 
            thinspace
            )
        )
    end
})


local volicon = wibox.widget.imagebox(theme.widget_volume)
local volume = lain.widget.pulse {
    settings = function()
        vlevel = 
            -- volume_now.device 
            -- .. 
            -- " "
            -- ..
            volume_now.left 
        if volume_now.muted == "yes" then
            vlevel = vlevel .. " M"
        end
            widget:set_markup(
            markup.fontfg(theme.font, theme.color_accent_1, thinspace .. vlevel .. " ") 
            .. 
            markup.fontfg(theme.font, theme.color_fg_base, mpd_now.volume .. " " ))
        -- widget:set_markup(markup.fontfg(theme.font, theme.color_accent_1, mpd_now.volume .. "%  "))
        if string.match(volume_now.device, "^alsa_output.usb*") then
            volicon:set_image(theme.widget_headphone)
        else
            volicon:set_image(theme.widget_volume)
        end
    end
}

-- CPU
local cpuicon = wibox.widget.imagebox(theme.widget_cpu)
local cpu = lain.widget.cpu({
    settings = function()
        local cpu_usage = cpu_now.usage
        
        -- Определение уровня загрузки
        local status, color
        if cpu_usage < 5 then
            status = "NON"
            color = "#111111"  -- Чёрный
        elseif cpu_usage <= 19 then
            status = "INS"
            color = theme.fg_normal  -- Синий
        elseif cpu_usage <= 39 then
            status = "LOW"
            color = "#00FF00"  -- Зелёный
        elseif cpu_usage <= 59 then
            status = "MED"
            color = "#FFFF00"  -- Жёлтый
        elseif cpu_usage <= 79 then
            status = "HIG"
            color = "#FFA500"  -- Оранжевый
        elseif cpu_usage >= 80 then
            status = "CRI"
            color = "#FF0000"  -- Красный
        else
            status = "ERR"
            color = theme.fg_normal  -- Стандартный цвет для ошибок
        end
        
        -- Установка текста и цвета
        widget:set_markup(markup.fontfg(theme.font, color, " " .. status .. " "))
    end
})


-- Temp
local tempicon = wibox.widget.imagebox(theme.widget_temp)
local temp = lain.widget.temp({
    settings = function()
        widget:set_markup(markup.fontfg(theme.font, theme.color_accent_2, thinspace .. coretemp_now .. "°"))   
--      widget:set_markup(markup.font(theme.font, " " .. coretemp_now .. "°C "))
    end
})

-- / fs
local fsicon = wibox.widget.imagebox(theme.widget_hdd)
theme.fs = lain.widget.fs({
    options  = "--exclude-type=tmpfs",
    notification_preset = { fg = theme.fg_normal, bg = theme.bg_normal, font = "xos4 Terminus 10" },
    settings = function()
        widget:set_markup(markup.fontfg(theme.font, "#80d9d8", fs_now.available_mb .. " MB"))
--      widget:set_markup(markup.font(theme.font, " " .. fs_now.available_gb .. "GB "))
    end
})

-- Battery
local baticon = wibox.widget.imagebox(theme.widget_battery)
local bat = lain.widget.bat({
    settings = function()
        if bat_now.status ~= "N/A" then
            if bat_now.ac_status == 1 then
                widget:set_markup(markup.font(theme.font, " AC "))
                baticon:set_image(theme.widget_ac)
                return
            elseif not bat_now.perc and tonumber(bat_now.perc) <= 5 then
                baticon:set_image(theme.widget_battery_empty)
            elseif not bat_now.perc and tonumber(bat_now.perc) <= 15 then
                baticon:set_image(theme.widget_battery_low)
            else
                baticon:set_image(theme.widget_battery)
            end
            widget:set_markup(markup.font(theme.font, " " .. bat_now.perc .. "% "))
        else
            widget:set_markup()
            baticon:set_image(theme.widget_ac)
        end
    end
})

-- Net
local neticon = wibox.widget.imagebox(theme.widget_net)
local net = lain.widget.net({
    settings = function()
        widget:set_markup(markup.fontfg(theme.font, "#FEFEFE", " " .. net_now.received .. " ↓↑ " .. net_now.sent .. " "))
    end
})
theme.weather = lain.widget.weather({
    city_id = 554233, -- placeholder (London)
    notification_preset = { font = theme.font },
    settings = function()
        units = math.floor(weather_now["main"]["temp"])
        -- icon_descr = "09n"
        widget:set_markup(markup.fontcolor(theme.font, theme.color_fg_base, theme.color_bg_base, " " .. units .. "°C "))
    end

})

local mylauncher = awful.widget.button({ image = theme.awesome_icon1, bgcolor="#36393f" })
mylauncher:connect_signal("button::press", function() awful.util.mymainmenu:toggle() end)

-- Separators
local arrow = separators.arrow_left

function theme.powerline_rl(cr, width, height) -- ИСХОДНИК В POWERARROWE
    local arrow_depth, offset = height/1.5, 0

    -- Avoid going out of the (potential) clip area
    if arrow_depth < 0 then
        width  =  width + 2*arrow_depth
        offset = -arrow_depth
    end

    cr:move_to( arrow_depth         , 0        )
    cr:line_to(offset + width               , 0        )
    cr:line_to(offset + width - arrow_depth , height )
    cr:line_to(offset + width               , height   )
    cr:line_to(0         , height   )
    cr:line_to(offset                       , height )

    cr:close_path()
end

local function pl(widget, bgcolor, padding)
    return wibox.container.background(wibox.container.margin(widget, 16, 16), bgcolor, theme.powerline_rl)
end

local function pl2(widget, bgcolor, padding)
    return wibox.container.background(wibox.container.margin(widget, 16, 0), bgcolor, theme.powerline_rl)
end

local function get_tag_type(t)
    if not t then return nil end

    if t.tag_type then
        return t.tag_type
    end

    local global_tag_types = rawget(_G, "awesome_tag_types") or {}
    local screen_index = t.screen and t.screen.index
    local tag_type = nil

    if screen_index and t.name then
        tag_type = global_tag_types[tostring(screen_index) .. ":" .. t.name]
    end

    if not tag_type and t.name then
        tag_type = global_tag_types[t.name]
    end

    return tag_type
end

local function get_tag_colors(t)
    if not t then
        return theme.tag_type_default.fg, theme.tag_type_default.bg
    end

    if t.selected then
        return theme.taglist_fg_focus or theme.fg_focus, theme.taglist_bg_focus or theme.bg_focus
    end

    if t.urgent then
        return theme.taglist_fg_urgent or theme.fg_urgent, theme.taglist_bg_urgent or theme.bg_urgent
    end

    local tag_type = get_tag_type(t)
    local colors = tag_type and theme.tag_type_colors[tag_type] or theme.tag_type_default

    return colors.fg or theme.tag_type_default.fg, colors.bg or theme.tag_type_default.bg
end

local function apply_typed_tag_style(self, t)
    local fg, bg = get_tag_colors(t)

    self.fg = fg
    self.bg = bg

    if self.set_fg then
        self:set_fg(fg)
    end

    if self.set_bg then
        self:set_bg(bg)
    end

    local text_widgets = self:get_children_by_id("text_role")
    local text_widget = text_widgets and text_widgets[1]

    if text_widget then
        local name = gears.string.xml_escape(t and t.name or "")
        text_widget.font = theme.taglist_font
        text_widget.markup = '<span foreground="' .. fg .. '" background="' .. bg .. '">' .. name .. '</span>'
    end
end

local function taglist_filter(t)
    if not t then
        return false
    end

    -- Не показываем служебные/скрытые теги.
    if t.hide then
        return false
    end

    -- Страховка именно для quake-like Kitty.
    if t.name == "kitty-dropdown" or t.name == "__KITTY_DROPDOWN__" then
        return false
    end

    return true
end

local function make_typed_taglist(s)
    return awful.widget.taglist {
        screen  = s,
        filter  = taglist_filter,
        buttons = awful.util.taglist_buttons,
        widget_template = {
            {
                {
                    id     = "text_role",
                    widget = wibox.widget.textbox,
                    align  = "center",
                    valign = "center",
                    font   = theme.taglist_font,
                },
                left   = 4,
                right  = 4,
                top    = 1,
                bottom = 1,
                widget = wibox.container.margin,
            },
            id     = "typed_background_role",
            widget = wibox.container.background,
            create_callback = function(self, t)
                apply_typed_tag_style(self, t)
            end,
            update_callback = function(self, t)
                apply_typed_tag_style(self, t)
            end,
        },
    }
end

function theme.at_screen_connect(s)
    -- Quake application
    s.quake = lain.util.quake({ app = awful.util.terminal })

    local wallpaper
    if s.index == 1 then
        wallpaper = theme.wall_center
    elseif s.index == 2 then
        wallpaper = theme.wall_right
    elseif s.index == 3 then
        wallpaper = theme.wall_left
    elseif s.index == 4 then
        wallpaper = theme.wall_touch
    else
        wallpaper = theme.wall_other
    end


    if type(wallpaper) == "function" then
        wallpaper = wallpaper(s)
    end
    gears.wallpaper.maximized(wallpaper, s, true)

    -- Tags
    awful.tag(awful.util.tagnames, s, awful.layout.layouts[1])

    -- Create a promptbox for each screen
    s.mypromptbox = awful.widget.prompt()
    -- Create an imagebox widget which will contains an icon indicating which layout we're using.
    -- We need one layoutbox per screen.
    s.mylayoutbox = awful.widget.layoutbox(s)
    s.mylayoutbox:buttons(awful.util.table.join(
                           awful.button({ }, 1, function () awful.layout.inc( 1) end),
                           awful.button({ }, 3, function () awful.layout.inc(-1) end)))
    -- Create a taglist widget
    s.mytaglist = make_typed_taglist(s)

    -- Create a tasklist widget
    s.mytasklist = awful.widget.tasklist(s, awful.widget.tasklist.filter.currenttags, awful.util.tasklist_buttons)


        -- РАЗДЕЛ ВИБОКСОВ
        -- ПОД РАЗНЫЕ ЭКРАНЫ


    -- ПЕРВЫЙ ЭКРАН


    if s.index == 1 then
   
    -- ВЕРХНИЙ ВИБОКС

    s.mywibox = awful.wibar({ position = "top", screen = s, height = 18, bg = theme.bg_normal, fg = theme.color_fg_bright })
    s.mywibox:setup {
        layout = wibox.layout.align.horizontal,
        { -- Left widgets
            layout = wibox.layout.fixed.horizontal,
            --spr,
            s.mytaglist,
            s.mypromptbox,
        },
        nil, -- Middle widgets
        { -- Right widgets
            layout = wibox.layout.fixed.horizontal,
        cpu_widget({
            width = 20,
            step_width = 2,
            enable_kill_button = true,
            step_spacing = 0,
            timeout = 1,
            -- color = '#0000'
        }),
        ram_widget({ 
            widget_show_buf = true, 
            color_used = theme.fg_normal, 
            color_free = '#000000', 
            color_buf = '#777777' 
        }), 
        fs_widget({ 
            mounts = {
                '/', 
                '/ward', 
                '/archive', 
                '/sync', 
                '/game', 
                '/ml', 
                '/media/win', 
                '/media/change'
            } 
        }),
        pl(wibox.widget {mytextclock, layout = wibox.layout.align.horizontal }, "#36393f"),
        wibox.widget.systray(),
        },
    }
    
    -- НИЖНИЙ ВИБОКС

    s.mybottomwibox = awful.wibar({ position = "bottom", screen = s, border_width = 0, height = 22, bg = theme.color_bg_base, fg = theme.fg_normal })

    -- Add widgets to the bottom wibox
    s.mybottomwibox:setup {
        layout = wibox.layout.align.horizontal,
        { -- Left widgets
            layout = wibox.layout.fixed.horizontal,
        pl(wibox.widget {s.mylayoutbox, layout = wibox.layout.fixed.horizontal }, "#36393f"),
        -- pl(wibox.widget { 
        --     mem.widget, 
        --     -- cpu.widget, 
        --     -- temp.widget, 
        -- layout = wibox.layout.fixed.horizontal },"#36393f"), 
        },
        -- Middle widgets 
        pl(s.mytasklist, theme.color_bg_base),
        { -- Right widgets
            layout = wibox.layout.fixed.horizontal,
        -- wibox.widget {countdown.widget, layout = wibox.layout.fixed.horizontal }, 
        -- wibox.widget {checkbox.widget, layout = wibox.layout.fixed.horizontal }, 
        -- wibox.widget {temp.widget, layout = wibox.layout.align.horizontal },     
        pl(wibox.widget { mpdicon, theme.mpd.widget, layout = wibox.layout.align.horizontal }, "#36393f"),
        wibox.widget {volicon, volume.widget, layout = wibox.layout.fixed.horizontal },
        pl(wibox.widget { theme.kbd.widget, layout = wibox.layout.align.horizontal }, "#36393f"),

        },
    }

    -- ВТОРОЙ ЭКРАН (ПРАВЫЙ)

    elseif s.index == 2 then
    
    -- ВЕРХНИЙ ВИБОКС

    s.mywibox = awful.wibar({ position = "top", screen = s, height = 18, bg = theme.bg_normal, fg = theme.color_fg_bright })

    s.mywibox:setup {
        layout = wibox.layout.align.horizontal,
        { -- Left widgets
            layout = wibox.layout.fixed.horizontal,
            s.mytaglist,
            s.mypromptbox,
        },
        nil,       
        { -- Right widgets
            layout = wibox.layout.fixed.horizontal,
        pl(wibox.widget { neticon, net_speed_widget(), layout = wibox.layout.align.horizontal }, "#36393f"),
        wibox.widget.textbox(" "),
        wibox.widget {theme.weather.icon, theme.weather.widget, layout = wibox.layout.align.horizontal},
        },
    }

    -- НИЖНИЙ ВИБОКС
    
    s.mybottomwibox = awful.wibar({ position = "bottom", screen = s, border_width = 0, height = 22, bg = theme.color_bg_base, fg = theme.fg_normal })

    -- Add widgets to the bottom wibox
    s.mybottomwibox:setup {
        layout = wibox.layout.align.horizontal,
        { -- Left widgets
            layout = wibox.layout.fixed.horizontal,
            caldav_tasks_widget.widget,
            pl(wibox.widget { 
                mem.widget,
               vram.widget,
            layout = wibox.layout.fixed.horizontal },"#36393f"), 
            wibox.widget { 
                cpu.widget, 
            layout = wibox.layout.fixed.horizontal },
            pl(wibox.widget { 
                temp.widget, 
            layout = wibox.layout.fixed.horizontal },"#36393f"), 
        },
        -- Middle widgety 
        pl(s.mytasklist, theme.color_bg_base),
        { -- Right widgets
            layout = wibox.layout.fixed.horizontal,
        -- wibox.widget {countdown.widget, layout = wibox.layout.fixed.horizontal }, nil,
        -- wibox.widget {checkbox.widget, layout = wibox.layout.fixed.horizontal }, nil,
        -- wibox.widget {temp.widget, layout = wibox.layout.align.horizontal }, nil,    
        -- pl(wibox.widget { mpdicon, theme.mpd.widget, layout = wibox.layout.align.horizontal }, "#36393f"),
        pl(wibox.widget {mylauncher,layout = wibox.layout.align.horizontal },"#36393f"),
        },
    }
    end
end

return theme