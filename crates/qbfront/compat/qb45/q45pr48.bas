' Parentheses and operator classes have distinct precedence witnesses.
dim firstValue as integer
dim secondValue as integer
dim thirdValue as integer

read firstValue, secondValue, thirdValue
if firstValue + secondValue * thirdValue <> 17 then
    print "FAIL precedence multiply"
    end
end if
if (firstValue + secondValue) * thirdValue <> 25 then
    print "FAIL precedence parentheses"
    end
end if
if 2 ^ secondValue * 4 <> 32 then
    print "FAIL precedence power"
    end
end if
if ((firstValue < secondValue) and (secondValue < thirdValue)) <> -1 then
    print "FAIL precedence relation"
    end
end if
print "PASS precedence"
end

data 2, 3, 5
