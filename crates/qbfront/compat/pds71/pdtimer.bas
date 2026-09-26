' The finite cap turns a missed event into a result, never a hung suite run.
dim hits as integer
dim value as long
dim attempts as long

hits = 0
value = 7
on timer(1) gosub eventHandler
timer on
for attempts = 1 to 20000000
    if hits <> 0 then exit for
next attempts
timer off

if hits = 1 and value = 99 then
    print "PASS pds-timer-event"
else
    print "FAIL pds-timer-event dispatch"
end if
end

eventHandler:
timer off
hits = hits + 1
value = 99
return
