' CLEAR resets scalars, strings, and static arrays and closes open files.
dim numberValue as long
dim textValue as string
dim fixedValues(1 to 3) as integer
dim errorNumber as integer
dim errorLine as integer
dim resumed as integer
dim fileText as string

numberValue = 81
textValue = "not empty"
fixedValues(1) = 11
fixedValues(2) = 22
fixedValues(3) = 33
open "CL66.DAT" for output as #1
print #1, chr$(numberValue) + left$(textValue, 1) + chr$(fixedValues(1) + fixedValues(2) + fixedValues(3));
clear

if numberValue <> 0 then
    print "FAIL clear number"
    end
end if
if textValue <> "" then
    print "FAIL clear string"
    end
end if
if fixedValues(1) <> 0 or fixedValues(2) <> 0 or fixedValues(3) <> 0 then
    print "FAIL clear array"
    end
end if

on error goto errorHandler
100 print #1, "X";
resumed = 1
on error goto 0
open "CL66.DAT" for input as #1
fileText = input$(lof(1), 1)
close #1
kill "CL66.DAT"
if errorNumber <> 52 or errorLine <> 100 or resumed <> 1 then
    print "FAIL clear close"; errorNumber; errorLine; resumed
    end
end if
if fileText <> "QnB" then
    print "FAIL clear flush"
    end
end if
print "PASS clear"
end

errorHandler:
errorNumber = err
errorLine = erl
resume next
