' QB45 compatibility source.
dim colorValue as integer

screen 1
cls
draw "c2r10d10"
colorValue = point(10, 10)
screen 0

if colorValue = 2 then
    print "PASS draw"
else
    print "FAIL draw endpoint"
end if
end
