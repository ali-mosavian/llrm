defint a-z

declare function parityAlgebra (a as integer, b as integer) as long
declare function parityAlgebraDemo () as long

print "RESULT="; parityAlgebraDemo()
print "DONE"
end

function parityAlgebra (a as integer, b as integer) as long
    dim value as long

    value = clng(a) * 9 + clng(b) * 5
    parityAlgebra = value * 3 - clng(a)
end function

function parityAlgebraDemo () as long
    parityAlgebraDemo = parityAlgebra(23, 7) * 1000 + parityAlgebra(-11, 4)
end function
