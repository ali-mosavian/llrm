' CINT and CLNG rounding and range are independent.
dim positiveValue as double
dim negativeValue as double
dim longValue as double

read positiveValue, negativeValue, longValue
if cint(positiveValue) <> 13 then
    print "FAIL conversions cint-positive"
    end
end if
if cint(negativeValue) <> -13 then
    print "FAIL conversions cint-negative"
    end
end if
if clng(longValue) <> 40000 then
    print "FAIL conversions clng-range"
    end
end if
print "PASS conversions"
end

data 12.6, -12.6, 40000.4
