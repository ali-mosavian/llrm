' ON ERROR, ERR, ERL, and RESUME NEXT restore ordinary flow.
dim handled as integer
dim resumed as integer

on error goto handler
100 error 11
resumed = 1
if handled <> 1 then
    print "FAIL resume handled"
    end
end if
if resumed <> 1 then
    print "FAIL resume flow"
    end
end if
print "PASS resume"
end

handler:
if err <> 11 then
    print "FAIL resume errnum"
    end
end if
if erl <> 100 then
    print "FAIL resume errline"
    end
end if
handled = 1
resume next
