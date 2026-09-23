' RND modes and RANDOMIZE are witnessed by measured IEEE SINGLE bytes.
dim seedValue as integer
dim firstValue as single
dim secondValue as single
dim resetValue as single
dim repeatValue as single
dim negativeSeedValue as single
dim negativeNextValue as single
dim negativeResetValue as single

read seedValue
randomize seedValue
firstValue = rnd
secondValue = rnd
randomize seedValue
resetValue = rnd
repeatValue = rnd(0)
negativeSeedValue = rnd(-7)
negativeNextValue = rnd
negativeResetValue = rnd(-7)
if mks$(firstValue) <> chr$(195) + chr$(229) + chr$(53) + chr$(63) then
    print "FAIL random first"
    end
end if
if mks$(secondValue) <> chr$(244) + chr$(114) + chr$(193) + chr$(62) then
    print "FAIL random second"
    end
end if
if mks$(resetValue) <> chr$(170) + chr$(152) + chr$(208) + chr$(62) then
    print "FAIL random reseed"
    end
end if
if mks$(repeatValue) <> chr$(170) + chr$(152) + chr$(208) + chr$(62) then
    print "FAIL random zero-repeat"
    end
end if
if mks$(negativeSeedValue) <> chr$(24) + chr$(228) + chr$(180) + chr$(61) then
    print "FAIL random negative-seed"
    end
end if
if mks$(negativeNextValue) <> chr$(58) + chr$(149) + chr$(9) + chr$(63) then
    print "FAIL random negative-next"
    end
end if
if mks$(negativeResetValue) <> chr$(24) + chr$(228) + chr$(180) + chr$(61) then
    print "FAIL random negative-reset"
    end
end if
print "PASS random"
end

data 1234
