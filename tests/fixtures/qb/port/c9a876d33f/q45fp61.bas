' Floating pack/unpack functions are checked against literal IEEE bytes.
dim singleValue as single
dim doubleValue as double
dim wholeValue as integer
dim quarterValue as double
dim singleText as string
dim doubleText as string

read singleValue, wholeValue, quarterValue
doubleValue = -cdbl(wholeValue) - quarterValue
singleText = mks$(singleValue)
if singleText <> chr$(0) + chr$(0) + chr$(192) + chr$(63) then
    print "FAIL floatpack single-bytes"
    end
end if
if cvs(singleText) <> 1.5 then
    print "FAIL floatpack single-value"
    end
end if
doubleText = mkd$(doubleValue)
if doubleText <> chr$(0) + chr$(0) + chr$(0) + chr$(0) + chr$(160) + chr$(28) + chr$(200) + chr$(192) then
    print "FAIL floatpack double-bytes"
    end
end if
if cvd(doubleText) <> doubleValue then
    print "FAIL floatpack double-value"
    end
end if
print "PASS floatpack"
end

data 1.5, 12345, .25
