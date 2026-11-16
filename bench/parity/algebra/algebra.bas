defint a-z

declare function parityAlgebra& (a as integer, b as integer)
declare function BenchAlgebra& ()

' Read from DATA so no build folds the kernel to a constant.
dim shared inA1 as integer, inB1 as integer, inA2 as integer, inB2 as integer
read inA1, inB1, inA2, inB2
data 23, 7, -11, 4

print ltrim$(str$(BenchAlgebra&))
end

function parityAlgebra& (a as integer, b as integer)
    dim value as long

    value = clng(a) * 9 + clng(b) * 5
    parityAlgebra& = value * 3 - clng(a)
end function

function BenchAlgebra&
    BenchAlgebra& = parityAlgebra&(inA1, inB1) * 1000 + parityAlgebra&(inA2, inB2)
end function
