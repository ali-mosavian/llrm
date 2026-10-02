' known: #358
' Six-body integrator in SINGLE (the fixed-point nbody_fixed state as floats), 1000 steps.
DEFINT A-Z
CONST BODIES = 6
DECLARE FUNCTION BenchNbodySingle& ()

DIM SHARED posX(BODIES) AS SINGLE
DIM SHARED posY(BODIES) AS SINGLE
DIM SHARED velX(BODIES) AS SINGLE
DIM SHARED velY(BODIES) AS SINGLE

PRINT LTRIM$(STR$(BenchNbodySingle&))
END

FUNCTION BenchNbodySingle&
    DIM deltaX AS SINGLE, deltaY AS SINGLE, dist2 AS SINGLE, falloff AS SINGLE
    DIM accX AS SINGLE, accY AS SINGLE, sum AS SINGLE
    FOR body = 0 TO BODIES - 1
        posX(body) = body * 7 - 15
        posY(body) = body * 5 - 12
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
                    dist2 = deltaX * deltaX + deltaY * deltaY + 1!
                    falloff = 1! / (dist2 + 1!)
                    accX = accX + deltaX * falloff
                    accY = accY + deltaY * falloff
                END IF
            NEXT
            velX(body) = velX(body) + accX
            velY(body) = velY(body) + accY
            velX(body) = velX(body) - velX(body) / 16!
            velY(body) = velY(body) - velY(body) / 16!
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
    BenchNbodySingle& = CLNG(sum * 1024!)
END FUNCTION
