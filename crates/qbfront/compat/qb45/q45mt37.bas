' Independent transcendental anchors use DATA operands and tolerances.
dim oneValue as double
dim halfPi as double
dim piValue as double
dim twoValue as double

read oneValue, halfPi, piValue, twoValue
if abs(atn(oneValue) - .7853981633974483) > .000000000001 then
    print "FAIL math atn"
    end
end if
if abs(sin(halfPi) - 1) > .000000000001 then
    print "FAIL math sin"
    end
end if
if abs(cos(piValue) + 1) > .000000000001 then
    print "FAIL math cos"
    end
end if
if abs(exp(oneValue) - 2.718281828459045) > .000000000001 then
    print "FAIL math exp"
    end
end if
if abs(log(twoValue) - .6931471805599453) > .000000000001 then
    print "FAIL math log"
    end
end if
print "PASS math"
end

data 1, 1.570796326794897, 3.141592653589793, 2
