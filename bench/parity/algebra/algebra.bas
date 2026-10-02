defint a-z

declare function parityAlgebra (a as integer, b as integer) as long
declare function BenchAlgebra& ()

print ltrim$(str$(BenchAlgebra&()))
end

function parityAlgebra (a as integer, b as integer) as long
    dim value as long

    value = clng(a) * 9 + clng(b) * 5
    parityAlgebra = value * 3 - clng(a)
end function

function BenchAlgebra& ()
    BenchAlgebra& = parityAlgebra(23, 7) * 1000 + parityAlgebra(-11, 4)
end function
