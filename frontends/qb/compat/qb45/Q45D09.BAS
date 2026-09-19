' QB45 compatibility source.
dim firstValue as integer
dim text as string
dim restoredValue as integer

read firstValue, text
restore secondData
read restoredValue

if firstValue = 7 and text = "seven" and restoredValue = 11 then
    print "PASS data"
else
    print "FAIL data restore"
end if
end

data 7, "seven"
secondData:
data 11
