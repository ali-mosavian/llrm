' Reduced QB/VBDOS executable form of FreeBASIC tests/qb/call.bas.
declare function subtractPair (byval leftValue as long, byval rightValue as long) as long

dim result as long
result = subtractPair(50, 8)

if result = 42 then
    print "CALL OK"
else
    print "CALL BAD"
end if

function subtractPair (byval leftValue as long, byval rightValue as long) as long
    subtractPair = leftValue - rightValue
end function
