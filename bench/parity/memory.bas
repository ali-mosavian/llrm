defint a-z

declare function parityMemory (value as integer, delta as integer) as long
declare function parityMemoryDemo () as long

print "RESULT="; parityMemoryDemo()
print "DONE"
end

function parityMemory (value as integer, delta as integer) as long
    value = value * 3 + delta
    parityMemory = clng(value) * clng(value)
end function

function parityMemoryDemo () as long
    parityMemoryDemo = parityMemory(7, -2) * 1000 + parityMemory(-4, 11)
end function
