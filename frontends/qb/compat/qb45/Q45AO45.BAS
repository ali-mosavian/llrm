' Arithmetic operators use nonconstant, noncommutative operands.
dim leftValue as integer
dim rightValue as integer
dim longValue as long

read leftValue, rightValue, longValue
if leftValue + rightValue <> 22 then
    print "FAIL arithmetic add"
    end
end if
if leftValue - rightValue <> 12 then
    print "FAIL arithmetic subtract"
    end
end if
if leftValue * rightValue <> 85 then
    print "FAIL arithmetic multiply"
    end
end if
if leftValue \ rightValue <> 3 or leftValue mod rightValue <> 2 then
    print "FAIL arithmetic divmod"
    end
end if
if longValue - leftValue * rightValue <> 99915 then
    print "FAIL arithmetic long"
    end
end if
print "PASS arithmetic"
end

data 17, 5, 100000
