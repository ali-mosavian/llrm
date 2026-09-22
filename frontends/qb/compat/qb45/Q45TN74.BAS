' TAN checks unrelated positive and negative DOUBLE inputs against numeric anchors.
dim positiveInput as double
dim negativeInput as double
dim positiveValue as double
dim negativeValue as double

read positiveInput, negativeInput
positiveValue = tan(positiveInput)
negativeValue = tan(negativeInput)
if abs(positiveValue - .5463024898437905) > .000000000001 then
    print "FAIL tangent positive"
    end
end if
if abs(negativeValue + .9315964599440725) > .000000000001 then
    print "FAIL tangent negative"
    end
end if
if tan(0) <> 0 then
    print "FAIL tangent zero"
    end
end if
print "PASS tangent"
end

data .5, -.75
