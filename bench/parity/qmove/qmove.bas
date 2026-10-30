' BC 4.5 takes no BYVAL on a BASIC procedure: arguments pass by reference, the C and Nib ones by value.
defint a-z

const PLACCELERATE = 10

type Vec3
    x as single
    y as single
    z as single
end type

declare sub plGroundAccel (vel as Vec3, wishdir as Vec3, wishspeed as single, dt as single)
declare function BenchQmove& ()

' Read from DATA so no build folds the kernel to a constant.
dim shared inVx as single, inVy as single, inWx as single, inSpeed as single, inDt as single
read inVx, inVy, inWx, inSpeed, inDt
data 3.0, 4.0, 1.0, 10.0, 0.25

print ltrim$(str$(BenchQmove&))
end

' From qb-qrender pl_move.bas and qcport game/pl_move.c.  This is Quake's
' SV_Accelerate arithmetic, including the cap as top speed is approached.
sub plGroundAccel (vel as Vec3, wishdir as Vec3, wishspeed as single, dt as single)
    dim currentspeed as single
    dim addspeed as single
    dim accelspeed as single

    currentspeed = vel.x * wishdir.x + vel.y * wishdir.y
    addspeed = wishspeed - currentspeed
    if addspeed <= 0.0 then exit sub

    accelspeed = PLACCELERATE * wishspeed * dt
    if accelspeed > addspeed then accelspeed = addspeed

    vel.x = vel.x + accelspeed * wishdir.x
    vel.y = vel.y + accelspeed * wishdir.y
end sub

function BenchQmove&
    dim vel as Vec3
    dim wishdir as Vec3

    vel.x = inVx
    vel.y = inVy
    vel.z = 5.0
    wishdir.x = inWx
    wishdir.y = 0.0
    wishdir.z = 0.0
    plGroundAccel vel, wishdir, inSpeed, inDt

    BenchQmove& = clng(vel.x) * 10000 + clng(vel.y) * 100 + clng(vel.z)
end function
