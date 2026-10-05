' Six-body Q9 fixed-point integrator, 1000 steps; LONG arithmetic, \ truncates toward zero.
DEFINT A-Z
CONST BODIES = 6
CONST ONE = 512&
CONST SOFTEN = ONE * ONE
CONST PULL = ONE
DECLARE FUNCTION BenchNbodyFixed& ()

DIM SHARED posX(BODIES) AS LONG
DIM SHARED posY(BODIES) AS LONG
DIM SHARED velX(BODIES) AS LONG
DIM SHARED velY(BODIES) AS LONG

PRINT LTRIM$(STR$(BenchNbodyFixed&))
END

FUNCTION BenchNbodyFixed&
    DIM deltaX AS LONG, deltaY AS LONG, dist2 AS LONG, falloff AS LONG
    DIM accX AS LONG, accY AS LONG, sum AS LONG
    FOR body = 0 TO BODIES - 1
        posX(body) = (body * 7 - 15) * ONE
        posY(body) = (body * 5 - 12) * ONE
        velX(body) = 0
        velY(body) = 0
    NEXT
    FOR stepNo = 1 TO 1000
        FOR body = 0 TO BODIES - 1
            accX = 0
            accY = 0
            FOR other = 0 TO BODIES - 1
                IF other <> body THEN
                    deltaX = posX(other) - posX(body)
                    deltaY = posY(other) - posY(body)
                    dist2 = deltaX * deltaX + deltaY * deltaY + SOFTEN
                    falloff = PULL \ (dist2 \ (ONE * ONE) + 1)
                    accX = accX + (deltaX * falloff) \ ONE
                    accY = accY + (deltaY * falloff) \ ONE
                END IF
            NEXT
            velX(body) = velX(body) + accX
            velY(body) = velY(body) + accY
            velX(body) = velX(body) - velX(body) \ 16
            velY(body) = velY(body) - velY(body) \ 16
        NEXT
        FOR body = 0 TO BODIES - 1
            posX(body) = posX(body) + velX(body)
            posY(body) = posY(body) + velY(body)
        NEXT
    NEXT
    sum = 0
    FOR body = 0 TO BODIES - 1
        sum = sum + (body + 1) * (posX(body) + 3 * posY(body) + 5 * velX(body) + 7 * velY(body))
    NEXT
    BenchNbodyFixed& = sum
END FUNCTION
