' BASIC logical operators are bitwise over signed INTEGER values.
dim leftValue as integer
dim rightValue as integer

read leftValue, rightValue
if (leftValue and rightValue) <> 8 then
    print "FAIL logical and"
    end
end if
if (leftValue or rightValue) <> 14 then
    print "FAIL logical or"
    end
end if
if (leftValue xor rightValue) <> 6 then
    print "FAIL logical xor"
    end
end if
if (leftValue eqv rightValue) <> -7 then
    print "FAIL logical eqv"
    end
end if
if (leftValue imp rightValue) <> -5 then
    print "FAIL logical imp"
    end
end if
if (not leftValue) <> -13 then
    print "FAIL logical not"
    end
end if
print "PASS logical"
end

data 12, 10
