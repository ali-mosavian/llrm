' QB 4.5 lexical and statement-separator baseline.
' A comment may follow an executable statement.
dim qCount as integer
dim qTotal as long
const limit = 7

screen 0
cls
def seg = &hb800
poke 0, 81: poke 1, 7
poke 2, 66: poke 3, 7
poke 4, 52: poke 5, 7
poke 6, 53: poke 7, 7
poke 8, 84: poke 9, 7
poke 10, 69: poke 11, 7
poke 12, 88: poke 13, 7
poke 14, 84: poke 15, 7
def seg
qCount = limit: qTotal = 100000 + qCount
' qCount = 99
rem qTotal = 1

if qCount = 7 and qTotal = 100007 then
    def seg = &hb800
    bsave "QBTEXT.BSV", 0, 4000
    def seg
    print "PASS text-screen-capture"
else
    print "FAIL text-screen-capture values"
end if
end
