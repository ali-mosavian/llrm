' Relational operators must produce BASIC -1/0 truth values.
dim lowValue as integer
dim highValue as integer
dim firstText as string
dim secondText as string

read lowValue, highValue, firstText, secondText
if (lowValue < highValue) <> -1 or (highValue < lowValue) <> 0 then
    print "FAIL relation less"
    end
end if
if (lowValue <= lowValue) <> -1 or (highValue >= lowValue) <> -1 then
    print "FAIL relation bounds"
    end
end if
if (lowValue = highValue) <> 0 or (lowValue <> highValue) <> -1 then
    print "FAIL relation equality"
    end
end if
if (firstText < secondText) <> -1 or (firstText = secondText) <> 0 then
    print "FAIL relation string"
    end
end if
print "PASS relation"
end

data -3, 7, "ABC", "ABD"
