defint a-z

declare function parityScalar () as long

print "RESULT="; parityScalar()
print "DONE"
end

function parityScalar () as long
    dim total as long
    dim index as integer

    total = 17
    for index = 0 to 7
        total = total + clng(index * 3 + 1) * (29 - index * 2)
    next index
    parityScalar = total
end function
