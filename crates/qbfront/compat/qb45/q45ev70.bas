' ENVIRON sets, replaces, and removes a value observed independently by ENVIRON$.
dim firstValue as string
dim secondValue as string
dim removedValue as string

environ "QBCOMPAT=Alpha42"
firstValue = environ$("QBCOMPAT")
environ "QBCOMPAT=Beta7"
secondValue = environ$("QBCOMPAT")
environ "QBCOMPAT="
removedValue = environ$("QBCOMPAT")

if firstValue <> "Alpha42" then
    print "FAIL environ first"
    end
end if
if secondValue <> "Beta7" then
    print "FAIL environ replace"
    end
end if
if removedValue <> "" then
    print "FAIL environ remove"
    end
end if
print "PASS environ"
end
