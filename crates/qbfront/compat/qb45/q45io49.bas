' WRITE # byte spelling and INPUT # parsing use different fixtures.
dim fileNumber as integer
dim rawText as string
dim parsedText as string
dim positiveValue as integer
dim negativeValue as integer

fileNumber = freefile
open "Q45IO49.DAT" for output as #fileNumber
write #fileNumber, "A,B", 42, -7
close #fileNumber
open "Q45IO49.DAT" for input as #fileNumber
rawText = input$(lof(fileNumber), fileNumber)
close #fileNumber
if rawText <> chr$(34) + "A,B" + chr$(34) + ",42,-7" + chr$(13) + chr$(10) then
    print "FAIL typedio write bytes"
    kill "Q45IO49.DAT"
    end
end if
open "Q45IO49.DAT" for output as #fileNumber
print #fileNumber, chr$(34); "C,D"; chr$(34); ",123,-45"
close #fileNumber
open "Q45IO49.DAT" for input as #fileNumber
input #fileNumber, parsedText, positiveValue, negativeValue
close #fileNumber
kill "Q45IO49.DAT"
if parsedText <> "C,D" or positiveValue <> 123 or negativeValue <> -45 then
    print "FAIL typedio input values"
    end
end if
print "PASS typedio"
end
