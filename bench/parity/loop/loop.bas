defint a-z

declare function parityLoop& (count as integer, seed as integer)
declare function BenchLoop& ()

' Read from DATA so no build folds the kernel to a constant.
dim shared countA as integer, seedA as integer, countB as integer, seedB as integer
read countA, seedA, countB, seedB
data 7, 5, 4, -3

print ltrim$(str$(BenchLoop&))
end

function parityLoop& (count as integer, seed as integer)
    dim total as long
    dim index as integer

    total = seed
    index = 0
    do while index < count
        total = total + clng(index) * seed + 3
        index = index + 1
    loop
    parityLoop& = total
end function

function BenchLoop&
    BenchLoop& = parityLoop&(countA, seedA) * 1000 + parityLoop&(countB, seedB)
end function
