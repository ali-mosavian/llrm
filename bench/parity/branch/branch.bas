defint a-z

declare function parityBranch& (value as integer)
declare function BenchBranch& ()

' Read from DATA so no build folds the kernel to a constant.
dim shared inputLow as integer, inputHigh as integer
read inputLow, inputHigh
data -13, 21

print ltrim$(str$(BenchBranch&))
end

function parityBranch& (value as integer)
    if value < 0 then
        parityBranch& = clng(value) * 7 + 3
    else
        parityBranch& = clng(value) * 5 - 9
    end if
end function

function BenchBranch&
    BenchBranch& = parityBranch&(inputLow) * 1000 + parityBranch&(inputHigh)
end function
