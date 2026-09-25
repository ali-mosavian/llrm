' STATIC procedures retain local arrays across calls.
' $STATIC
declare function nextValue () as integer
dim firstValue as integer
dim secondValue as integer

firstValue = nextValue()
secondValue = nextValue()
if firstValue = 1 and secondValue = 2 then
    print "PASS memorymodel"
else
    print "FAIL memorymodel lifetime"
end if
end

function nextValue () as integer static
    dim counts(0 to 0) as integer
    counts(0) = counts(0) + 1
    nextValue = counts(0)
end function
