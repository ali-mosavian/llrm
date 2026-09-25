defint a-z

type Vec2i
    x as long
    y as long
end type

type Body
    pos as Vec2i
    vel as Vec2i
end type

declare function walk () as long

dim shared answer as long
answer = walk()
end

function walk () as long static
    dim bodies(0 to 99) as Body
    dim current as integer

    for current = 0 to 99
        bodies(current).pos.x = bodies(current).vel.x
    next current
    walk = bodies(99).pos.x
end function
