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
print "HITS="; hits
print "VALUE="; value
print "DONE"
end

eventHandler:
timer off
hits = hits + 1
value = 99
return
