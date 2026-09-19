defint a-z

type Coord
    x as integer
    y as integer
end type

declare function parityKernel () as long

print "RESULT="; parityKernel()
print "DONE"
end

function parityKernel () as long
    dim points(0 to 7) as Coord
    dim total as long
    dim index as integer

    total = 17
    for index = 0 to 7
        points(index).x = index * 3 + 1
        points(index).y = 29 - index * 2
    next index
    for index = 0 to 7
        total = total + clng(points(index).x) * points(index).y
    next index
    parityKernel = total
end function
