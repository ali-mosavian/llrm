defint a-z

declare function BenchScalar& (seed as integer)

print ltrim$(str$(BenchScalar&(cint(val("0")))))
end

function BenchScalar& (seed as integer)
    dim total as long
    dim index as integer

    total = 17
    for index = 0 to 7
        total = total + clng(index * 3 + 1 + seed) * (29 - index * 2)
    next index
    BenchScalar& = total
end function
