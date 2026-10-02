defint a-z

declare function BenchScalar& ()

print ltrim$(str$(BenchScalar&()))
end

function BenchScalar& ()
    dim total as long
    dim index as integer

    total = 17
    for index = 0 to 7
        total = total + clng(index * 3 + 1) * (29 - index * 2)
    next index
    BenchScalar& = total
end function
