' QB45 compatibility source.
dim eventCount as integer

on strig(0) gosub strigTick
strig(0) on
strig(0) off
gosub strigTick

if eventCount = 1 then
    print "PASS strigevent"
else
    print "FAIL strigevent registration"
end if
end

strigTick:
eventCount = eventCount + 1
return
