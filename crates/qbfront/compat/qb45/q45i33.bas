' Include directive occupies the end of its source line.
dim includeValue as integer
' $INCLUDE: 'q45i33.inc'

if includeValue = 17 then
    print "PASS include"
else
    print "FAIL include value"
end if
end
