' Negative DOUBLE SQR returns an unordered NaN without a BASIC error.
dim inputValue as double
dim resultValue as double

on error goto unexpectedError
read inputValue
100 resultValue = sqr(inputValue)
if resultValue = resultValue then
    print "FAIL sqrt-negative ordered"
    end
end if
print "PASS sqrt-negative"
end

unexpectedError:
print "FAIL sqrt-negative error"; err; erl
end

data -1
