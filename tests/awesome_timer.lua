local response, last_command, notices, polls = '', '', {}, 0
local map = {}
local theme = {font='System Theme Font 11', color_accent_3='#112233', color_accent_2='#aabbcc', color_fg_base='#ddeeff'}
local timers = {}
package.preload['beautiful'] = function() return theme end
package.path = '/usr/share/awesome/lib/?.lua;/usr/share/awesome/lib/?/init.lua;' .. package.path
awesome = {connect_signal=function() end, emit_signal=function() end}
package.preload['lain.helpers'] = function() return {
    set_map=function(k,v) map[k]=v end, get_map=function(k) return map[k] end,
    async=function(cmd, callback) polls=polls+1;last_command=cmd[3];callback(response) end,
    newtimer=function() error('Shared timer must not be used') end,
} end
package.preload['awful.util'] = function() return {shell='/bin/sh',escape=function(s) return s:gsub('&','&amp;'):gsub('<','&lt;'):gsub('>','&gt;') end} end
package.preload['awful.screen'] = function() return {focused=function() return 1 end} end
package.preload['naughty'] = function() return {notify=function(n) notices[#notices+1]=n.preset.text; return {id=1} end} end
package.preload['wibox'] = function() return {widget={textbox=function() return {set_markup=function(self,value) self.markup=value end} end}} end

response='state: play\nfile: https://radio.test/live\nName: Station\nTitle: First\nComment: mopytui-radio\n'
local factory=dofile(arg[1])
local applet=factory({notify='off'})
assert(polls == 1)
local GLib=require('lgi').GLib
local loop=GLib.MainLoop()
GLib.timeout_add(GLib.PRIORITY_DEFAULT, 150, function()
    response=response:gsub('Title: First', 'Title: Second')
    return false
end)
GLib.timeout_add(GLib.PRIORITY_DEFAULT, 1200, function() loop:quit(); return false end)
loop:run()
applet.timer:stop()
assert(polls >= 3, 'automatic timer did not keep polling')
assert(applet.widget.markup:find('Second', 1, true), applet.widget.markup)
print('PASS: actual gears.timer/GLib loop updates track without clicks ('..polls..' polls)')
