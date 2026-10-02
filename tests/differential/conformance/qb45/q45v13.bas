' QB45 compatibility source.
dim colorValue as integer

screen 1
cls
pset (10, 10), 2
colorValue = point(10, 10)
screen 0

if colorValue = 2 then
    print "PASS display"
else
    print "FAIL display point"
end if
end
