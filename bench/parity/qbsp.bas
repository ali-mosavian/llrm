defint a-z

type Vec3
    x as single
    y as single
    z as single
end type

type Plane
    norm as Vec3
    dist as single
end type

type Node
    planeId as integer
    child0 as integer
    child1 as integer
end type

declare function rPlaneDist (p as Vec3, pl as Plane) as single
declare function rPointLeaf (p as Vec3, nodes() as Node, planes() as Plane) as integer
declare function quakeBspDemo () as long

print "RESULT="; quakeBspDemo()
print "DONE"
end

' From qb-qrender r_bsp.bas and qcport render/r_bsp.c.
function rPlaneDist (p as Vec3, pl as Plane) as single
    rPlaneDist = p.x * pl.norm.x + p.y * pl.norm.y + p.z * pl.norm.z - pl.dist
end function

' Walk hull zero until the sign-bit leaf marker is reached.
function rPointLeaf (p as Vec3, nodes() as Node, planes() as Plane) as integer
    dim nodenr as integer

    nodenr = 0
    do while (nodenr and &h8000) = 0
        if rPlaneDist(p, planes(nodes(nodenr).planeId)) >= 0.0 then
            nodenr = nodes(nodenr).child0
        else
            nodenr = nodes(nodenr).child1
        end if
    loop

    rPointLeaf = not nodenr
end function

function quakeBspDemo () as long
    dim p as Vec3
    dim nodes(0 to 1) as Node
    dim planes(0 to 1) as Plane
    dim a as integer
    dim b as integer
    dim c as integer

    planes(0).norm.x = 1.0
    planes(1).norm.y = 1.0
    nodes(0).planeId = 0
    nodes(0).child0 = 1
    nodes(0).child1 = -1
    nodes(1).planeId = 1
    nodes(1).child0 = -2
    nodes(1).child1 = -3

    p.x = 2.0
    p.y = 3.0
    a = rPointLeaf(p, nodes(), planes())
    p.y = -3.0
    b = rPointLeaf(p, nodes(), planes())
    p.x = -2.0
    c = rPointLeaf(p, nodes(), planes())

    quakeBspDemo = clng(a) * 100 + clng(b) * 10 + c
end function
