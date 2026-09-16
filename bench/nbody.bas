' The same fixed-point integrator as suite/nbody.bas, timed with RDTSC.
' bench/tscsnap.asm returns EDX:EAX as two LONG values; tools/bench.py
' combines them into an unsigned 64-bit cycle count.
defint a-z
const BODIES = 6
const ONE = 512&
const SOFTEN = ONE * ONE
const PULL = ONE

dim posX(BODIES) as long
dim posY(BODIES) as long
dim velX(BODIES) as long
dim velY(BODIES) as long
dim deltaX as long
dim deltaY as long
dim dist2 as long
dim falloff as long
dim accX as long
dim accY as long
dim stepCount as long
dim stepNo as long
dim body as integer
dim other as integer
dim tag as string
dim hiStart as long
dim loStart as long
dim hiEnd as long
dim loEnd as long

declare sub TscSnap (hi as long, lo as long)

stepCount = val(command$)
if stepCount <= 0 then stepCount = 100

for body = 0 to BODIES - 1
    posX(body) = (body * 7 - 15) * ONE
    posY(body) = (body * 5 - 12) * ONE
    velX(body) = 0
    velY(body) = 0
next

call TscSnap(hiStart, loStart)

for stepNo = 1 to stepCount
    for body = 0 to BODIES - 1
        accX = 0
        accY = 0
        for other = 0 to BODIES - 1
            if other <> body then
                deltaX = posX(other) - posX(body)
                deltaY = posY(other) - posY(body)
                dist2 = deltaX * deltaX + deltaY * deltaY + SOFTEN
                falloff = PULL \ (dist2 \ (ONE * ONE) + 1)
                accX = accX + (deltaX * falloff) \ ONE
                accY = accY + (deltaY * falloff) \ ONE
            end if
        next
        velX(body) = velX(body) + accX
        velY(body) = velY(body) + accY
        velX(body) = velX(body) - velX(body) \ 16
        velY(body) = velY(body) - velY(body) \ 16
    next
    for body = 0 to BODIES - 1
        posX(body) = posX(body) + velX(body)
        posY(body) = posY(body) + velY(body)
    next
next

call TscSnap(hiEnd, loEnd)

for body = 0 to BODIES - 1
    tag = ltrim$(str$(body))
    print "PX" + tag + "="; posX(body)
    print "PY" + tag + "="; posY(body)
    print "VX" + tag + "="; velX(body)
    print "VY" + tag + "="; velY(body)
next
print "TSC0="; hiStart; loStart
print "TSC1="; hiEnd; loEnd
print "DONE"
end
