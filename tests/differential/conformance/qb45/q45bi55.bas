' Binary PUT/GET positions and layout are checked independently.
dim fileNumber as integer
dim firstValue as integer
dim secondValue as integer
dim readFirst as integer
dim readSecond as integer
dim rawText as string

read firstValue, secondValue
fileNumber = freefile
open "Q45BI55.DAT" for binary as #fileNumber
put #fileNumber, 1, firstValue
put #fileNumber, 3, secondValue
if lof(fileNumber) <> 4 then
    print "FAIL binaryio length"
    close #fileNumber
    kill "Q45BI55.DAT"
    end
end if
if seek(fileNumber) <> 5 then
    print "FAIL binaryio seek-after-put"
    close #fileNumber
    kill "Q45BI55.DAT"
    end
end if
get #fileNumber, 3, readSecond
if loc(fileNumber) <> 4 then
    print "FAIL binaryio loc-after-get"
    close #fileNumber
    kill "Q45BI55.DAT"
    end
end if
get #fileNumber, 1, readFirst
close #fileNumber
if readFirst <> firstValue or readSecond <> secondValue then
    print "FAIL binaryio values"
    kill "Q45BI55.DAT"
    end
end if
open "Q45BI55.DAT" for input as #fileNumber
rawText = input$(lof(fileNumber), fileNumber)
close #fileNumber
kill "Q45BI55.DAT"
if rawText <> chr$(52) + chr$(18) + chr$(254) + chr$(255) then
    print "FAIL binaryio bytes"
    end
end if
print "PASS binaryio"
end

data &H1234, -2
