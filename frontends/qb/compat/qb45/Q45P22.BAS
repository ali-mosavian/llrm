' QB45 compatibility source.
dim eventCount as integer

on play(1) gosub playTick
play on
play off
gosub playTick

if eventCount = 1 then
    print "PASS playevent"
else
    print "FAIL playevent registration"
end if
end

playTick:
eventCount = eventCount + 1
return
