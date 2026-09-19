defint a-z

declare function parityLoop (count as integer, seed as integer) as long
declare function parityLoopDemo () as long

print "RESULT="; parityLoopDemo()
print "DONE"
end

function parityLoop (count as integer, seed as integer) as long
    dim total as long
    dim index as integer

    total = seed
    index = 0
    do while index < count
        total = total + clng(index) * seed + 3
        index = index + 1
    loop
    parityLoop = total
end function

function parityLoopDemo () as long
    parityLoopDemo = parityLoop(7, 5) * 1000 + parityLoop(4, -3)
end function
