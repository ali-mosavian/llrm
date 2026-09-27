' QB45 compatibility source.
dim ticks as integer

on timer(86400) gosub timerTick
timer on
timer off
gosub timerTick

if ticks = 1 then
    print "PASS events"
else
    print "FAIL events registration"
end if
end

timerTick:
ticks = ticks + 1
return
