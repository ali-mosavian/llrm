' The same integrator as bench/nbody.bas, in SINGLE rather than fixed point,
' and timed the same way: the 8253 PIT, not TIMER. What it exists to measure
' is the floating-point emulator, which is what BC compiles every one of
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

dim tStart as long
dim cStart as long
dim tEnd as long
dim cEnd as long
dim elapsed as long

declare sub PitSnap (tk as long, ctr as long)

stepCount = val(command$)
if stepCount <= 0 then stepCount = 100

for body = 0 to BODIES - 1
    posX(body) = (body * 7 - 15)
    posY(body) = (body * 5 - 12)
    velX(body) = 0
    velY(body) = 0
next

call PitSnap(tStart, cStart)

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

call PitSnap(tEnd, cEnd)
elapsed = (tEnd - tStart) * 65536& + (cStart - cEnd)

for body = 0 to BODIES - 1
    tag = ltrim$(str$(body))
    print "PX" + tag + "="; clng(posX(body) * 1000)
    print "PY" + tag + "="; clng(posY(body) * 1000)
next
print "TICKS="; elapsed
print "DONE"
end

sub PitSnap (tk as long, ctr as long)
    ' Masking IRQ0 at the 8259 (BASIC has no CLI) did not stop a one-period
    ' tear between the tick at 0040:006C and the down-counter: measured across
    ' repeated identical runs, it kept happening anyway, which means the
    ' emulator updates that memory location on its own schedule rather than by
    ' actually delivering IRQ0 to a handler a mask could hold off. What is
    ' left is a guard band: refuse any reading within GUARD units of either
    ' edge of the counter's own period, since that edge is the only place a
    ' tear between the two values could land, and take another right away.
    const GUARD = 12000
    dim mask as integer
    dim lo as integer
    dim hi as integer
    dim raw as long
    do
        mask = inp(&H21)
        out &H21, mask or &H1
        out &H43, &H0
        lo = inp(&H40)
        hi = inp(&H40)
        def seg = &H40
        tk = clng(peek(&H6C)) + clng(peek(&H6D)) * 256& + clng(peek(&H6E)) * 65536& + clng(peek(&H6F)) * 16777216&
        out &H21, mask
        raw = clng(lo) + clng(hi) * 256&
    loop while raw > 65536& - GUARD or raw < GUARD
    ctr = raw
end sub
