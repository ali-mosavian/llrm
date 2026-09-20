' QB45 compatibility source.
dim eventCount as integer

on com(1) gosub comTick
com(1) on
com(1) off
gosub comTick

if eventCount = 1 then
    print "PASS comevent"
else
    print "FAIL comevent registration"
end if
end

comTick:
eventCount = eventCount + 1
return
