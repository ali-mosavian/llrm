' FIELD, LSET, and fixed-length RANDOM record transfer.
dim fileNumber as integer
dim passed as integer

fileNumber = freefile
open "Q45F31.DAT" for random as #fileNumber len = 4
field #fileNumber, 4 as textBuffer$
lset textBuffer$ = "AB"
put #fileNumber, 1
lset textBuffer$ = ""
get #fileNumber, 1
passed = textBuffer$ = "AB  " and lof(fileNumber) = 4
close #fileNumber
kill "Q45F31.DAT"

if passed then
    print "PASS fieldio"
else
    print "FAIL fieldio record"
end if
end
