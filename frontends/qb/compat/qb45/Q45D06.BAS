' $dynamic
dim index as integer
dim total as long

redim cacheList(2 to 4) as long
for index = 2 to 4
    cacheList(index) = index * 100000
    total = total + cacheList(index)
next index

erase cacheList
redim cacheList(-1 to 1) as long
cacheList(-1) = 17
cacheList(0) = 23
cacheList(1) = 29
if total = 900000 and lbound(cacheList) = -1 and ubound(cacheList) = 1 and cacheList(-1) + cacheList(0) + cacheList(1) = 69 then
    print "PASS dynamicarrays"
else
    print "FAIL dynamicarrays total"
end if
end
