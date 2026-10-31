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
local applet=factory({timeout=60, settings=function() widget:set_markup('ordinary layout') end})
assert(polls==1, 'initial automatic poll missing')
response='state: play\nfile: https://radio.test/live\nName: Station <A>\nArtist: Artist & Band\nTitle: Track\nComment: mopytui-radio\n'
timers[1].callback()
assert(applet.widget.markup=='<span foreground="#aabbcc">Station &lt;A&gt;</span><span foreground="#ddeeff"> · </span><span foreground="#112233">Artist &amp; Band</span><span foreground="#ddeeff"> — Track</span>',applet.widget.markup)
assert(applet.widget.font==theme.font)
assert(not applet.widget.markup:find('font=') and not applet.widget.markup:find('size='))
assert(last_command:find("socket.create_connection",1,true))
response=response:gsub('Title: Track','Title: Changed without click')
timers[1].callback()
assert(applet.widget.markup:find('Changed without click',1,true), 'timer did not update track')
local long_name=string.rep('Ж',31)..'&'..'discarded'
response='state: play\nfile: https://radio.test/live\nName: '..long_name..'\nArtist: \nTitle: '..long_name..'\nComment: mopytui-radio\n'
timers[1].callback()
assert(applet.widget.markup=='<span foreground="#aabbcc">'..string.rep('Ж',31)..'&amp;</span>',applet.widget.markup)
assert(mpd_now.station:find('discarded',1,true), 'full station should remain in data')
response='state: pause\nfile: https://music.test/track\nArtist: Ordinary\nTitle: Song\n'
timers[1].callback();assert(applet.widget.markup=='<span foreground="#ddeeff">Пауза</span>');assert(not mpd_now.radio)
local second=factory({})
assert(second.timer~=applet.timer, 'instances must have separate timers')
print('PASS: theme font/colors, UTF-8 32-character limit, escaping, automatic polls/track updates, independent timers, ordinary layout')
