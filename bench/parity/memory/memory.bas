defint a-z

declare function parityMemory (value as integer, delta as integer) as long
declare function BenchMemory& ()

print ltrim$(str$(BenchMemory&()))
end

function parityMemory (value as integer, delta as integer) as long
    value = value * 3 + delta
    parityMemory = clng(value) * clng(value)
end function

function BenchMemory& ()
    BenchMemory& = parityMemory(7, -2) * 1000 + parityMemory(-4, 11)
end function
