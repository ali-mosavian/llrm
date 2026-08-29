' A 32-bit fixed-point n-body integrator: what a DOS program does when it has
' real arithmetic to do and no coprocessor to do it with. Every quantity is a
' long in 16.16, so the inner loop is long subtract, multiply, divide, add and
' compare and nothing else -- runs of them, which is the shape the pass exists
' for and the shape qb-qrender turned out not to have.
'
' Two multiplies, because 32 bits will not hold a 16.16 product directly:
'
'     both operands large     (a \ 256) * (b \ 256)
'     the second one small    ((a \ 256) * b) \ 256
'
' Shifting both operands down by eight annihilates one that is under 1.0, and
' shifting neither overflows. Which form applies is decided by magnitude, which
' is the arithmetic a 386 leaves to whoever is writing it. The largest
' intermediate any of this reaches is 13 per cent of a long.
'
' The step count comes from the command line, so one program serves as both the
' differential's case and the benchmark's: no argument means 100 steps, which
' is what tools/mkgolden.py authors.
defint a-z
const BODIES = 6
const ONE = 65536
const SOFTEN = 65536
const PULL = 65536

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
                dist2 = (deltaX \ 256) * (deltaX \ 256) + (deltaY \ 256) * (deltaY \ 256) + SOFTEN
                falloff = PULL \ (dist2 \ ONE + 1)
                accX = accX + ((deltaX \ 256) * falloff) \ 256
                accY = accY + ((deltaY \ 256) * falloff) \ 256
            end if
        next
        velX(body) = velX(body) + accX
        velY(body) = velY(body) + accY
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
