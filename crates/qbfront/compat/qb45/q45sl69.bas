' A STATIC local persists and shadows a same-named shared module variable.
defint n
declare function nextLocal(markerValue as integer)

dim shared counter as integer
dim firstValue as integer
dim secondValue as integer
dim thirdValue as integer
dim firstMarker as integer
dim secondMarker as integer
dim thirdMarker as integer

counter = 99
read firstMarker, secondMarker, thirdMarker
firstValue = nextLocal(firstMarker)
secondValue = nextLocal(secondMarker)
thirdValue = nextLocal(thirdMarker)
if firstValue <> 11 or secondValue <> 22 or thirdValue <> 33 then
    print "FAIL static sequence"
    end
end if
if counter <> 99 then
    print "FAIL static shadow"
    end
end if
print "PASS static-local"
end

function nextLocal(markerValue as integer)
    static counter as integer
    counter = counter + 1
    nextLocal = markerValue + counter
end function

data 10, 20, 30
