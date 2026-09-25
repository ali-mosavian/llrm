' CINT and CLNG use nearest-even rounding at exact half boundaries.
dim lowPositive as double
dim highPositive as double
dim lowNegative as double
dim highNegative as double

read lowPositive, highPositive, lowNegative, highNegative
if cint(lowPositive) <> 12 then
    print "FAIL round-ties cint-low-positive"
    end
end if
if cint(highPositive) <> 14 then
    print "FAIL round-ties cint-high-positive"
    end
end if
if cint(lowNegative) <> -12 then
    print "FAIL round-ties cint-low-negative"
    end
end if
if cint(highNegative) <> -14 then
    print "FAIL round-ties cint-high-negative"
    end
end if
if clng(lowPositive) <> 12 then
    print "FAIL round-ties clng-low-positive"
    end
end if
if clng(highPositive) <> 14 then
    print "FAIL round-ties clng-high-positive"
    end
end if
if clng(lowNegative) <> -12 then
    print "FAIL round-ties clng-low-negative"
    end
end if
if clng(highNegative) <> -14 then
    print "FAIL round-ties clng-high-negative"
    end
end if
print "PASS round-ties"
end

data 12.5, 13.5, -12.5, -13.5
