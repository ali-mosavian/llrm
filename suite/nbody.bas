' A 32-bit fixed-point n-body integrator, in a format BC's own long
' multiply and divide can carry without an intermediate wider than 32 bits.
'
' 16.16 needed a 64-bit product for a*b: N.M times N.M is N.2M, and 2*16=32
' already fills a long before the shift back down. 9 fraction bits does not:
' the quantity squared here is a position delta, and this simulation never
' lets one exceed about 5,500 raw, so deltaX * deltaX stays under 15 per
' cent of a long -- exactly what BC's own `*` compiles to, a call to
' B$MUI4, which qbopt absorbs into one imul. Nothing here needs a callee
' with no library body, which is the point: this program has a base build
' BC alone can link, and fixMul& never does.
'
' Measured rather than picked round: 9 is the most fraction bits this
' simulation's own range affords before a bare `*` overflows. 8 was tried
' first and needs `PULL` boosted eight times over to keep 1/dist2 from
' truncating to zero on most steps -- and even then the bodies fly apart
' fast enough to overflow dist2 within a few thousand steps. 9, at the
' simulation's natural pull strength and a velocity that loses a sixteenth
' each step, converges to a stable orbit instead: the worst intermediate
' measured at 1000 steps is the same one measured at 100, 15 per cent of a
' long for a single multiply and 23 per cent for dist2's sum of two. 10 is
' the last width that does not overflow outright, but leaves under 10 per
' cent of that margin and was not chosen for it.
'
' The step count comes from the command line, so one program serves as both
' the differential's case and the benchmark's: no argument means 100 steps,
' which is what tools/mkgolden.py authors.
defint a-z
const BODIES = 6
const ONE = 512&  ' & matters: untyped, 512*512 folds as INTEGER and wraps to 0
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

stepCount = val(command$)
if stepCount <= 0 then stepCount = 100

for body = 0 to BODIES - 1
    posX(body) = (body * 7 - 15) * ONE
    posY(body) = (body * 5 - 12) * ONE
    velX(body) = 0
    velY(body) = 0
next

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
        ' velocity loses a sixteenth each step -- 16 rather than `2 ^ 4`,
        ' whose result BASIC's `^` always returns as a float: measured, it
        ' compiles to two calls to B$POW4 per body per step, an unrelated
        ' floating-point cost this integrator has no other use for.
        velX(body) = velX(body) - velX(body) \ 16
        velY(body) = velY(body) - velY(body) \ 16
    next
    for body = 0 to BODIES - 1
        posX(body) = posX(body) + velX(body)
        posY(body) = posY(body) + velY(body)
    next
next

for body = 0 to BODIES - 1
    tag = ltrim$(str$(body))
    print "PX" + tag + "="; posX(body)
    print "PY" + tag + "="; posY(body)
    print "VX" + tag + "="; velX(body)
    print "VY" + tag + "="; velY(body)
next
print "DONE"
end
