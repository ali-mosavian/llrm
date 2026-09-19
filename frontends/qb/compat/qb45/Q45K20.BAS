' QB45 compatibility source.
dim eventCount as integer

on key(1) gosub keyTick
key(1) on
key(1) off
gosub keyTick

if eventCount = 1 then
    print "PASS keyevent"
else
    print "FAIL keyevent registration"
end if
end

keyTick:
eventCount = eventCount + 1
return
