' The same integrator as bench/nbody.bas, in SINGLE rather than fixed point,
' and timed with RDTSC. What it exists to measure is the floating-point
' emulator, which is what BC compiles every one of
' these operations into under /FPi -- an `int 34h`..`3Bh` per operation, a
' trap into a software routine. qbopt --native-fpu replaces each with the
' x87 instruction it stands for, and this is the program that says what that
' is worth.
'
' Deliberately the same shape as nbody so the two are comparable: same body
' count, same loop structure, same number of operations per step.
defint a-z
const BODIES = 6

dim posX(BODIES) as single
dim posY(BODIES) as single
dim velX(BODIES) as single
dim velY(BODIES) as single
dim deltaX as single
dim deltaY as single
dim dist2 as single
dim falloff as single
dim accX as single
dim accY as single
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
    posX(body) = (body * 7 - 15)
    posY(body) = (body * 5 - 12)
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
                dist2 = deltaX * deltaX + deltaY * deltaY + 1
                falloff = 1 / dist2
                accX = accX + deltaX * falloff
                accY = accY + deltaY * falloff
            end if
        next
        velX(body) = velX(body) + accX
        velY(body) = velY(body) + accY
        velX(body) = velX(body) - velX(body) / 16
        velY(body) = velY(body) - velY(body) / 16
    next
    for body = 0 to BODIES - 1
        posX(body) = posX(body) + velX(body)
        posY(body) = posY(body) + velY(body)
    next
next

call TscSnap(hiEnd, loEnd)

for body = 0 to BODIES - 1
    tag = ltrim$(str$(body))
    print "PX" + tag + "="; clng(posX(body) * 1000)
    print "PY" + tag + "="; clng(posY(body) * 1000)
next
print "TSC0="; hiStart; loStart
print "TSC1="; hiEnd; loEnd
print "DONE"
end
