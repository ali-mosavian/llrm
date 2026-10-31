defint a-z

declare function parityControl& (value as integer, limit as integer)
declare function BenchControl& ()

' Read from DATA so no build folds the kernel to a constant.
dim shared inputValueA as integer, inputLimitA as integer, inputValueB as integer, inputLimitB as integer
read inputValueA, inputLimitA, inputValueB, inputLimitB
data 7, 6, -3, 5

print ltrim$(str$(BenchControl&))
end

function parityControl& (value as integer, limit as integer)
    dim total as long
    dim index as integer

    total = 0
    for index = 0 to limit - 1
        if (index and 1) = 0 then
            total = total + clng(value) + index
        else
            total = total - clng(value) + index
        end if
    next index
    parityControl& = total
end function

function BenchControl&
    BenchControl& = parityControl&(inputValueA, inputLimitA) * 1000 + parityControl&(inputValueB, inputLimitB)
end function
