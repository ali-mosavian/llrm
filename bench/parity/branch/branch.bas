defint a-z

declare function parityBranch& (value as integer)
declare function BenchBranch& ()

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
    BenchBranch& = parityBranch&(-13) * 1000 + parityBranch&(21)
end function
