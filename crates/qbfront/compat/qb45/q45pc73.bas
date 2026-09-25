' SHELL executes one deterministic child command and returns after its file closes.
dim fileNumber as integer
dim fileText as string
dim errorNumber as integer
dim errorLine as integer

on error goto errorHandler
100 shell "command /c echo CHILD>SH73.DAT"
fileNumber = freefile
open "SH73.DAT" for binary as #fileNumber
if lof(fileNumber) <> 7 then
    print "FAIL shell length"
    close #fileNumber
    end
end if
fileText = input$(7, fileNumber)
close #fileNumber
kill "SH73.DAT"
if fileText <> "CHILD" + chr$(13) + chr$(10) then
    print "FAIL shell bytes"
    end
end if
print "PASS shell"
end

errorHandler:
errorNumber = err
errorLine = erl
print "FAIL shell error"; errorNumber; errorLine
end
