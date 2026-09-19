defint a-z

const PLACCELERATE = 10

type Vec3
    x as single
    y as single
    z as single
end type

declare sub plGroundAccel (vel as Vec3, wishdir as Vec3, byval wishspeed as single, byval dt as single)
declare function quakeMoveDemo () as long

print "RESULT="; quakeMoveDemo()
print "DONE"
end

' From qb-qrender pl_move.bas and qcport game/pl_move.c.  This is Quake's
' SV_Accelerate arithmetic, including the cap as top speed is approached.
sub plGroundAccel (vel as Vec3, wishdir as Vec3, byval wishspeed as single, byval dt as single)
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

function quakeMoveDemo () as long
    dim vel as Vec3
    dim wishdir as Vec3

    vel.x = 3.0
    vel.y = 4.0
    vel.z = 5.0
    wishdir.x = 1.0
    wishdir.y = 0.0
    wishdir.z = 0.0
    plGroundAccel vel, wishdir, 10.0, 0.25

    quakeMoveDemo = clng(vel.x) * 10000 + clng(vel.y) * 100 + clng(vel.z)
end function
