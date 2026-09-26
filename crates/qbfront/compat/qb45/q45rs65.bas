' RESET flushes and closes every file channel.
dim errorNumber as integer
dim errorLine as integer
dim resumed as integer
dim fileText as string

on error goto errorHandler
open "RS65.DAT" for output as #1
print #1, "AB";
reset
100 print #1, "X";
resumed = 1
on error goto 0

open "RS65.DAT" for input as #1
fileText = input$(lof(1), 1)
close #1
kill "RS65.DAT"
if errorNumber <> 52 or errorLine <> 100 or resumed <> 1 then
    print "FAIL reset error"; errorNumber; errorLine; resumed
    end
end if
if fileText <> "AB" then
    print "FAIL reset flush"
    end
end if
print "PASS reset"
end

errorHandler:
errorNumber = err
errorLine = erl
resume next
