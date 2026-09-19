defint a-z

declare function parityBranch (value as integer) as long
declare function parityBranchDemo () as long

print "RESULT="; parityBranchDemo()
print "DONE"
end

function parityBranch (value as integer) as long
    if value < 0 then
        parityBranch = clng(value) * 7 + 3
    else
        parityBranch = clng(value) * 5 - 9
    end if
end function

function parityBranchDemo () as long
    parityBranchDemo = parityBranch(-13) * 1000 + parityBranch(21)
end function
