' APPEND, INPUT$, SEEK, EOF, and LOF use exact file positions.
dim fileNumber as integer
dim firstText as string
dim secondText as string

fileNumber = freefile
open "Q45FI39.DAT" for output as #fileNumber
print #fileNumber, "AB";
close #fileNumber
open "Q45FI39.DAT" for append as #fileNumber
print #fileNumber, "CD";
close #fileNumber
open "Q45FI39.DAT" for input as #fileNumber
if lof(fileNumber) <> 4 then
    print "FAIL filepos lof"
    close #fileNumber
    kill "Q45FI39.DAT"
    end
end if
firstText = input$(2, fileNumber)
seek #fileNumber, 3
secondText = input$(2, fileNumber)
if firstText <> "AB" or secondText <> "CD" or not eof(fileNumber) then
    print "FAIL filepos seek"
    close #fileNumber
    kill "Q45FI39.DAT"
    end
end if
close #fileNumber
kill "Q45FI39.DAT"
print "PASS filepos"
end
