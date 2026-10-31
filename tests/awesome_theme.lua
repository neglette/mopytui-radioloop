local response, last_command, notices, polls = '', '', {}, 0
local map = {}
local theme = {font='System Theme Font 11', color_accent_3='#112233', color_accent_2='#aabbcc', color_fg_base='#ddeeff'}
local timers = {}
package.preload['beautiful'] = function() return theme end
package.preload['gears.timer'] = function() return function(options)
    assert(options.autostart and options.call_now and options.timeout == 0.5)
    timers[#timers+1]=options
    options.callback()
    return options
end end
package.preload['lain.helpers'] = function() return {
    set_map=function(k,v) map[k]=v end, get_map=function(k) return map[k] end,
    async=function(cmd, callback) polls=polls+1;last_command=cmd[3];callback(response) end,
    newtimer=function() error('Shared timer must not be used') end,
} end
package.preload['awful.util'] = function() return {shell='/bin/sh',escape=function(s) return s:gsub('&','&amp;'):gsub('<','&lt;'):gsub('>','&gt;') end} end
package.preload['awful.screen'] = function() return {focused=function() return 1 end} end
package.preload['naughty'] = function() return {notify=function(n) notices[#notices+1]=n.preset.text; return {id=1} end} end
package.preload['wibox'] = function() return {widget={textbox=function() return {set_markup=function(self,value) self.markup=value end} end}} end

local factory=dofile(arg[1])
local escapes=require('awful.util').escape
local awful={util={escape=escapes,table={join=function(...) return {...} end}},button=function(...) return {...} end,spawn={easy_async=function(command, callback) callback() end}}
local icons={}
local wibox={widget={imagebox=function(image) local icon={image=image,set_image=function(self,v) self.image=v end,buttons=function() end}; icons[#icons+1]=icon;return icon end}}
local markup=setmetatable({font=function(font,text) return '<span font_desc="'..font..'">'..text..'</span>' end},{__call=function(_,color,text) return '<span foreground="'..color..'">'..text..'</span>' end})
local file=assert(io.open(arg[2]));local source=file:read('*a');file:close()
local block=assert(source:match('(%-%- MPD:.-)%-%- MEM'))
local chunk=assert(loadstring('local theme,lain,awful,wibox,markup,thinspace=...; '..block..'; return theme.mpd'))
theme.widget_music_net='radio';theme.widget_music_on='play';theme.widget_music_pause='pause';theme.widget_music_stop='stop'
local applet=chunk(theme,{widget={mpd=factory}},awful,wibox,markup,' ')
response='state: play\nfile: https://radio.test/live\nName: Station\nArtist: Artist\nTitle: No hyphen in title\nComment: mopytui-radio\n'
timers[1].callback()
assert(applet.widget.markup:find('No hyphen in title',1,true))
assert(applet.widget.markup:find('<span foreground="#112233">Artist</span>',1,true))
assert(applet.widget.markup:find('font_desc="System Theme Font 11"',1,true))
response=response:gsub('No hyphen in title','Next track without click');timers[1].callback()
assert(applet.widget.markup:find('Next track without click',1,true))
for state,label in pairs({pause='Пауза',stop='Выключено',offline='Нет соединения'}) do
 response='state: '..state..'\nName: Old station\nTitle: Stale track\nComment: mopytui-radio\n'
 timers[1].callback()
 assert(applet.widget.markup:find(label,1,true),applet.widget.markup)
 assert(not applet.widget.markup:find('Stale track',1,true))
end
print('PASS: supplied theme MPD block + widget; title without dash, automatic changes, accent 3, theme font, pause/stop/offline clearing')
