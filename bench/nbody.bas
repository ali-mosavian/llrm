' The same fixed-point integrator as suite/nbody.bas, timed with the 8253
' PIT rather than TIMER: docs/measurement.md's method, latch channel 0 and
' read the counter twice, retrying if the BIOS tick at 0040:006C changes
' between the two reads so a snapshot is never split across a rollover.
'
' Absolute PIT-tick*65536+counter overflows a LONG well within a session --
' the BIOS tick is time-of-day, not time-since-boot, and DOSBox mirrors the
' host clock. Only the two snapshots' difference is ever multiplied by 65536,
' which a bench run of any sane length keeps well inside range.
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
dim tStart as long
dim cStart as long
dim tEnd as long
dim cEnd as long
dim elapsed as long

declare sub PitSnap (tk as long, ctr as long)

stepCount = val(command$)
if stepCount <= 0 then stepCount = 100

for body = 0 to BODIES - 1
    posX(body) = (body * 7 - 15) * ONE
    posY(body) = (body * 5 - 12) * ONE
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

call PitSnap(tEnd, cEnd)
elapsed = (tEnd - tStart) * 65536& + (cStart - cEnd)

for body = 0 to BODIES - 1
    tag = ltrim$(str$(body))
    print "PX" + tag + "="; posX(body)
    print "PY" + tag + "="; posY(body)
    print "VX" + tag + "="; velX(body)
    print "VY" + tag + "="; velY(body)
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
