' $STATIC arrays retain allocation across ERASE and ignore REDIM.
' $STATIC
dim cacheList(0 to 2) as integer
dim passed as integer

cacheList(1) = 9
erase cacheList
passed = cacheList(1) = 0 and ubound(cacheList) = 2

if passed then
    print "PASS staticdynamic"
else
    print "FAIL staticdynamic allocation"
end if
end
