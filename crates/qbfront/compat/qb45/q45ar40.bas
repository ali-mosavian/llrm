' Signed array bounds and every boundary element are observable.
dim values(-2 to 2) as integer
dim index as integer

for index = -2 to 2
    values(index) = index * 11 + 100
next index
if lbound(values) <> -2 or ubound(values) <> 2 then
    print "FAIL arraybounds bounds"
    end
end if
if values(-2) <> 78 or values(-1) <> 89 or values(0) <> 100 or values(1) <> 111 or values(2) <> 122 then
    print "FAIL arraybounds values"
    end
end if
print "PASS arraybounds"
end
