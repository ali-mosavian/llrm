' QB45 compatibility source.
dim fileNumber as integer
dim text as string

fileNumber = freefile
open "q45f10.tmp" for output as #fileNumber
print #fileNumber, "QB45 FILE"
close #fileNumber

open "q45f10.tmp" for input as #fileNumber
line input #fileNumber, text
close #fileNumber
kill "q45f10.tmp"

if text = "QB45 FILE" then
    print "PASS files"
else
    print "FAIL files roundtrip"
end if
end
