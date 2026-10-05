dim items(1) as integer
dim handled as integer

on error goto boundsError
100 items(2) = 7
print "FAIL pds-runtime-check no-error"
end

boundsError:
if err = 9 and erl = 100 then handled = -1
resume finished

finished:
if handled then
    print "PASS pds-runtime-check"
else
    print "FAIL pds-runtime-check err"
end if
end
