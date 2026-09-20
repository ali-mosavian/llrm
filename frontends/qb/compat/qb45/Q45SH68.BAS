' A procedure SHARED statement aliases the module variable without a parameter.
declare sub updateShared(observedValue as integer)

dim shared sharedValue as integer
dim observedValue as integer

read sharedValue
call updateShared(observedValue)
if sharedValue <> 53 then
    print "FAIL shared module"
    end
end if
if observedValue <> 53 then
    print "FAIL shared witness"
    end
end if
print "PASS shared"
end

sub updateShared(observedValue as integer)
    shared sharedValue as integer
    sharedValue = sharedValue * 10 + 3
    observedValue = sharedValue
end sub

data 5
