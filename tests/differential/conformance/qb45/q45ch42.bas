' Character and case/trim functions have separate witnesses.
dim letter as string
dim mixedText as string
dim paddedText as string

read letter, mixedText, paddedText
if asc(letter) <> 90 then
    print "FAIL charfunc asc"
    end
end if
if chr$(65) <> "A" then
    print "FAIL charfunc chr"
    end
end if
if ucase$(mixedText) <> "MIXED" then
    print "FAIL charfunc ucase"
    end
end if
if lcase$(mixedText) <> "mixed" then
    print "FAIL charfunc lcase"
    end
end if
if ltrim$(paddedText) <> "tail  " then
    print "FAIL charfunc ltrim"
    end
end if
if rtrim$(paddedText) <> "  tail" then
    print "FAIL charfunc rtrim"
    end
end if
print "PASS charfunc"
end

data "Z", "MiXeD", "  tail  "
