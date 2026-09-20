' QB45 compatibility source.

on error goto handler
error 13
print "FAIL errors no-trap"
end

handler:
if err = 13 then
    print "PASS errors"
else
    print "FAIL errors code"
end if
end
