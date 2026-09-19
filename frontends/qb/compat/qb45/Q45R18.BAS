' QB45 compatibility source.
type SampleItem
    count as integer
    labelText as string * 6
end type

dim firstItem as SampleItem
dim secondItem as SampleItem
dim fileNumber as integer

firstItem.count = 7
firstItem.labelText = "record"
fileNumber = freefile
open "Q45R18.DAT" for random as #fileNumber len = len(firstItem)
put #fileNumber, 1, firstItem
get #fileNumber, 1, secondItem
close #fileNumber
kill "Q45R18.DAT"

if secondItem.count = 7 and secondItem.labelText = "record" then
    print "PASS randomio"
else
    print "FAIL randomio record"
end if
end
