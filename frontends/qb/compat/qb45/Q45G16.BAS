' QB45 compatibility source.
dim colorValue as integer

screen 13
cls
pset (0, 0), 1
pset (319, 199), 2
colorValue = point(0, 0) + point(319, 199)
def seg = &ha000
bsave "Q45G13.BSV", 0, 64000
def seg
screen 0

if colorValue = 3 then
    print "PASS graphics"
else
    print "FAIL graphics pixels"
end if
end
