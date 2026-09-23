' DOS directory and rename operations must affect later independent opens.
dim fileNumber as integer
dim renamedText as string

on error goto fileFailure
100 mkdir "Q45D59"
110 chdir "Q45D59"
fileNumber = freefile
open "MARK.DAT" for output as #fileNumber
print #fileNumber, "renamed"
close #fileNumber
120 chdir ".."
130 name "Q45D59\MARK.DAT" as "Q45D59\REN.DAT"
open "Q45D59\REN.DAT" for input as #fileNumber
line input #fileNumber, renamedText
close #fileNumber
if renamedText <> "renamed" then
    print "FAIL dirops renamed-data"
    end
end if
140 kill "Q45D59\REN.DAT"
150 rmdir "Q45D59"
print "PASS dirops"
end

fileFailure:
print "FAIL dirops runtime"; err; erl
end
