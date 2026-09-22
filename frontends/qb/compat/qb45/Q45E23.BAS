' QB45 compatibility source.
dim eventCount as integer

on pen gosub penTick
pen on
pen off
gosub penTick

if eventCount = 1 then
    print "PASS penevent"
else
    print "FAIL penevent registration"
end if
end

penTick:
eventCount = eventCount + 1
return
