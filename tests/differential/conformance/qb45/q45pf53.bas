' PRINT USING is witnessed through exact file bytes, not its own parser.
dim fileNumber as integer
dim rawText as string
dim value as double

on error goto runtimeFailure
read value
fileNumber = freefile
open "Q45PF53.DAT" for output as #fileNumber
100 print #fileNumber, using "###.##"; value
close #fileNumber
open "Q45PF53.DAT" for input as #fileNumber
rawText = input$(lof(fileNumber), fileNumber)
close #fileNumber
kill "Q45PF53.DAT"
if rawText <> " 12.50" + chr$(13) + chr$(10) then
    print "FAIL print-using bytes"
    end
end if
print "PASS print-using"
end

runtimeFailure:
print "FAIL print-using runtime"; err; erl
end

data 12.5
