' Include directive occupies the end of its source line.
dim includeValue as integer
' $INCLUDE: 'Q45I33.INC'

if includeValue = 17 then
    print "PASS include"
else
    print "FAIL include value"
end if
end
