' Numeric functions use independent DATA-fed inputs and checkpoints.
dim positiveValue as single
dim negativeValue as single
dim squareValue as single

read positiveValue, negativeValue, squareValue
if abs(negativeValue) <> 7.75 then
    print "FAIL numfunc abs"
    end
end if
if sgn(negativeValue) <> -1 or sgn(0) <> 0 or sgn(positiveValue) <> 1 then
    print "FAIL numfunc sgn"
    end
end if
if fix(negativeValue) <> -7 then
    print "FAIL numfunc fix"
    end
end if
if int(negativeValue) <> -8 then
    print "FAIL numfunc int"
    end
end if
if sqr(squareValue) <> 9 then
    print "FAIL numfunc sqr"
    end
end if
print "PASS numfunc"
end

data 2.25, -7.75, 81
